param([string]$Root, [string]$Executable, [uint32]$Seed = 74,
    [ValidateSet('hills','planet')][string]$World = 'hills', [uint32]$GeneratorVersion = 272)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\MeridianReviewCommands.ps1"
$repo = Split-Path $PSScriptRoot -Parent
if (!$Executable) { $Executable = Join-Path $repo 'target\release-fast\minecraft_style_voxel_prototype.exe' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (!$Root) { $Root = Join-Path $repo ('target\express-lift\run-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')) }
if (Test-Path -LiteralPath $Root) { throw 'Use a fresh disposable review directory.' }
New-Item -ItemType Directory -Path $Root | Out-Null
$Root = (Resolve-Path -LiteralPath $Root).Path
function Wait-Lift([string]$Pattern, [int]$Seconds = 120) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        if ($process.HasExited) { throw 'Native lift review exited early.' }
        if ((Test-Path -LiteralPath "$Root\native.log") -and
            ((Get-Content -LiteralPath "$Root\native.log" -Raw) -match $Pattern)) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Pattern"
}
function Shot([string]$Name, [int]$Settle = 3) {
    Start-Sleep -Seconds $Settle
    Write-MeridianCommand $Root "$Name capture"
    Wait-Lift ([regex]::Escape("$Name.png"))
}
try {
    $env:VOXEL_RECEIVER_REVIEW = '1'
    $env:VOXEL_CAPTURE_DIR = $Root
    $env:VOXEL_SAVE_DIR = Join-Path $Root 'disposable-saves'
    Remove-Item Env:VOXEL_REVIEW_NO_ORBITAL -ErrorAction SilentlyContinue
    Write-MeridianCommand $Root '00 report'
    $arguments = @("--benchmark-world=$World", "--benchmark-seed=$Seed", "--benchmark-generator-version=$GeneratorVersion", '--benchmark-weather=clear', '--benchmark-view-chunks=6',
        '--benchmark-width=1280', '--benchmark-height=900', '--benchmark-duration=3600', '--benchmark-capture-hud')
    $process = Start-Process -FilePath $Executable -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$Root\native.log" -RedirectStandardError "$Root\native.err.log"
    Wait-Lift '00 report stage=0'
    Wait-Lift '\[stream-profile\].*missing=0 .*horizon=true' 300
    Write-MeridianCommand $Root '01 lift-pose'
    Wait-Lift 'posed=true'
    Shot 'lift-before-berth' 4
    Write-MeridianCommand $Root '02 lift-board'
    Wait-Lift 'boarded=true'
    Shot 'lift-00-berth' 4
    Write-MeridianCommand $Root '40 lift-outside'
    Wait-Lift 'outside=true'
    Shot 'lift-outside' 4
    Write-MeridianCommand $Root '41 lift-walk 0 0'
    Wait-Lift 'walked=\(0\.0,0\.0\) deck=false'
    Write-MeridianCommand $Root '42 lift-look 0.35 -0.12'
    $i = 1
    foreach ($t in @(8, 25, 55, 85, 112, 125)) {
        Write-MeridianCommand $Root "$($i + 2) lift-time $t"
        Wait-Lift "trip_s=$t\.0"
        Shot ('lift-{0:D2}-t{1}' -f $i, $t) 6
        $i++
    }
    Write-MeridianCommand $Root '20 lift-look 2.6 -0.35'
    Shot 'lift-top-window' 3
    # Through the crown door onto the deck, then out along the gallery.
    Write-MeridianCommand $Root '30 lift-walk 2.6 0'
    Wait-Lift 'walked=\(2\.6'
    Write-MeridianCommand $Root '31 lift-look 1.5708 -0.05'
    Shot 'lift-door' 3
    Write-MeridianCommand $Root '32 lift-walk 9 -6'
    Wait-Lift 'walked=\(9\.0'
    Write-MeridianCommand $Root '33 lift-look -2.2 -0.1'
    Shot 'lift-deck-hub' 3
    # Inside the habitat drum (local x 2..31 m), past the 11 m gravity seam.
    Write-MeridianCommand $Root '34 lift-walk 26 0'
    Wait-Lift 'walked=\(26\.0'
    Write-MeridianCommand $Root '35 lift-look 1.5708 0.05'
    Shot 'lift-gallery-space' 3
    Write-MeridianCommand $Root '36 lift-look 1.5708 -0.75'
    Shot 'lift-gallery-planet' 3
    Write-MeridianCommand $Root '37 lift-look 0.3 0.35'
    Shot 'lift-gallery-up' 3
    Write-MeridianCommand $Root '38 lift-walk 0 0'
    Wait-Lift 'walked=\(0\.0'
    Write-MeridianCommand $Root '39 lift-look 0.35 -0.12'
    Write-MeridianCommand $Root '21 lift-descend'
    Wait-Lift 'descend=true'
    Write-MeridianCommand $Root '22 lift-time 90'
    Wait-Lift 'trip_s=90\.0'
    Shot 'lift-return-approach' 8
    Write-MeridianCommand $Root '23 lift-time 126'
    Wait-Lift 'trip_s=126\.0'
    Shot 'lift-released' 4
    [pscustomobject]@{executable=$Executable;sha256=(Get-FileHash $Executable -Algorithm SHA256).Hash;evidence=$Root} |
        ConvertTo-Json | Set-Content "$Root\result.json"
} finally {
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
