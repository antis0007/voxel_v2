param([string]$Root, [string]$Executable, [uint32]$Seed = 74, [switch]$AnchorageOnly,
    [switch]$MegastructuresOnly, [switch]$NormalRenderer, [switch]$Night, [ValidateSet('clear','storm')][string]$Weather='clear')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\MeridianReviewCommands.ps1"
$descentRepo = Split-Path $PSScriptRoot -Parent
if (!$Executable) { $Executable = Join-Path $descentRepo 'target\release-fast\minecraft_style_voxel_prototype.exe' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (!$Root) { $Root = Join-Path $descentRepo ('target\meridian-reviews\descent-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')) }
if (Test-Path -LiteralPath $Root) { throw 'Use a fresh disposable review directory.' }
New-Item -ItemType Directory -Path $Root | Out-Null
$Root = (Resolve-Path -LiteralPath $Root).Path
$descentEnvironment = @{}
foreach ($key in @('VOXEL_RECEIVER_REVIEW','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_REVIEW_NO_ORBITAL')) {
    $descentEnvironment[$key] = [Environment]::GetEnvironmentVariable($key)
}
function Wait-Descent([string]$Pattern) {
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    do {
        if ($descentProcess.HasExited) { throw 'Native descent review exited early.' }
        if ((Test-Path -LiteralPath "$Root\native.log") -and
            ((Get-Content -LiteralPath "$Root\native.log" -Raw) -match $Pattern)) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Pattern"
}
try {
    $env:VOXEL_RECEIVER_REVIEW = '1'
    $env:VOXEL_CAPTURE_DIR = $Root
    $env:VOXEL_SAVE_DIR = Join-Path $Root 'disposable-saves'
    Remove-Item Env:VOXEL_REVIEW_NO_ORBITAL -ErrorAction SilentlyContinue
    Write-MeridianCommand $Root '00 report'
    $descentArgs=@('--benchmark-world=hills',"--benchmark-seed=$Seed", "--benchmark-weather=$Weather",'--benchmark-view-chunks=3',
          '--benchmark-width=1280','--benchmark-height=900','--benchmark-duration=3600')
    if (!$NormalRenderer) {$descentArgs+=@('--benchmark-generator-version=25','--benchmark-no-indirect')}
    $descentProcess = Start-Process -FilePath $Executable `
        -ArgumentList $descentArgs `
        -WorkingDirectory $descentRepo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$Root\native.log" -RedirectStandardError "$Root\native.err.log"
    Wait-Descent '00 report stage=0'
    Wait-Descent '\[stream-profile\].*missing=0 .*horizon=true'
    if ($MegastructuresOnly) {
        $clock = if ($Night) {'megastructure-night'} else {'megastructure-day'}
        Write-MeridianCommand $Root "clock $clock"
        Wait-Descent "clock $clock stage=0"
    } elseif ($Night) {
        Write-MeridianCommand $Root 'night orbital-night'
        Wait-Descent 'night orbital-night stage=0'
    }
    if (!$AnchorageOnly -and !$MegastructuresOnly) {
      Write-MeridianCommand $Root '01 orbital-flight'
      Wait-Descent '01 orbital-flight stage=0'
      foreach ($frame in 0..5) {
        if ($frame -gt 0) { Start-Sleep -Seconds 6 }
        $name = 'descent-{0:D2}' -f $frame
        Write-MeridianCommand $Root "$name capture"
        Wait-Descent ([regex]::Escape("$name.png"))
      }
    }
    $shots = if ($MegastructuresOnly) { @('megastructure-foundry','megastructure-habitat','megastructure-crown') } elseif ($AnchorageOnly) { @('orbital-anchorage','orbital-anchorage-sweep') } else { @('orbital-ground','orbital-detail','orbital-anchorage','orbital-anchorage-sweep') }
    foreach ($shot in $shots) {
        Write-MeridianCommand $Root "$shot $shot"
        Wait-Descent "$shot $shot stage=0"
        Start-Sleep -Seconds 2
        Write-MeridianCommand $Root "$shot-image capture"
        Wait-Descent ([regex]::Escape("$shot-image.png"))
        if ($shot.StartsWith('orbital-anchorage')) {
            Start-Sleep -Seconds 4
            Write-MeridianCommand $Root "$shot-rotation capture"
            Wait-Descent ([regex]::Escape("$shot-rotation.png"))
        }
        if ($shot -eq 'megastructure-habitat') {
            Start-Sleep -Seconds 5
            Write-MeridianCommand $Root "$shot-rotation capture"
            Wait-Descent ([regex]::Escape("$shot-rotation.png"))
            foreach ($phase in @('orbit-before','orbit-after','orbit-hidden')) {
                Write-MeridianCommand $Root "$phase $phase"
                Wait-Descent "$phase $phase stage=0"
                Start-Sleep -Seconds 2
                Write-MeridianCommand $Root "$phase-image capture"
                Wait-Descent ([regex]::Escape("$phase-image.png"))
            }
        }
    }
    Set-Content -LiteralPath "$Root\receipt.txt" -Value "Native orbital review; anchorage_only=$AnchorageOnly; normal_renderer=$NormalRenderer; night=$Night; weather=$Weather; seed=$Seed. Disposable saves. Arguments: $descentArgs"
    Write-Output $Root
}
finally {
    if ($descentProcess -and !$descentProcess.HasExited) { Stop-Process -Id $descentProcess.Id; $descentProcess.WaitForExit() }
    foreach ($key in $descentEnvironment.Keys) { [Environment]::SetEnvironmentVariable($key,$descentEnvironment[$key]) }
}
