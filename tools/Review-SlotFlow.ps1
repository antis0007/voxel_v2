param(
    [string]$Executable = "$PSScriptRoot/../target/debug/minecraft_style_voxel_prototype.exe",
    [string]$Lobby = "$PSScriptRoot/../target/debug/voxel-lobby.exe",
    [string]$Output = "$PSScriptRoot/../target/slot-flow-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [int]$Port = 27138,
    [string]$NativeEvidence
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot/..").Path
$exe = (Resolve-Path -LiteralPath $Executable).Path
$relay = (Resolve-Path -LiteralPath $Lobby).Path
if (Test-Path -LiteralPath $Output) { throw 'Use a new evidence directory.' }
$root = (New-Item -ItemType Directory -Path $Output).FullName
$children = @()
$oldSave = $env:VOXEL_SAVE_DIR
$oldReview = $env:VOXEL_REVIEW_SLOT_FLOW

function Start-ReviewProcess($Name, $File, $Arguments) {
    $env:VOXEL_SAVE_DIR = (New-Item -ItemType Directory -Path (Join-Path $root $Name)).FullName
    Start-Process -FilePath $File -ArgumentList $Arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $root "$Name.stdout.log") `
        -RedirectStandardError (Join-Path $root "$Name.stderr.log")
}

function Wait-Receipt($Process, $File, $Pattern, $Seconds) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $match = Select-String -LiteralPath $File -Pattern $Pattern -ErrorAction SilentlyContinue | Select-Object -Last 1
        if ($match) { return $match }
        if ($Process.HasExited) { break }
        Start-Sleep -Milliseconds 250
    }
    throw "Missing receipt '$Pattern' in $File"
}

try {
    $env:VOXEL_REVIEW_SLOT_FLOW = '1'
    if (!$NativeEvidence) {
        $NativeEvidence = Join-Path $root 'native'
        & "$PSScriptRoot/Review-StorageScanner.ps1" -Executable $exe -Output $NativeEvidence -Width 1280 -Height 720
    }
    $build = (Get-FileHash -LiteralPath $exe).Hash
    $manifest = Get-Content -LiteralPath (Join-Path $NativeEvidence 'manifest.json') -Raw | ConvertFrom-Json
    if ($manifest.sha256 -ne $build) { throw 'Native evidence belongs to a different build.' }
    $build | Set-Content -LiteralPath (Join-Path $root 'game-sha256.txt')
    $local = @(Select-String -LiteralPath (Join-Path $NativeEvidence 'stdout.log') -SimpleMatch '[slot-flow-native]')
    if ($local.Count -lt 2) { throw 'Missing native chest or station receipt.' }
    $local | ForEach-Object Line

    $address = "127.0.0.1:$Port"
    $children += Start-ReviewProcess 'lobby' $relay @('--bind', $address)
    $hostPeer = Start-ReviewProcess 'host' $exe @('--multiplayer-headless', "--multiplayer-lobby=$address",
        '--multiplayer-online=SlotReview', '--multiplayer-ephemeral', '--multiplayer-review-arena',
        '--multiplayer-review-slots', '--multiplayer-run-seconds=100')
    $children += $hostPeer
    $listed = Wait-Receipt $hostPeer (Join-Path $root 'host.stderr.log') 'listed code=(\S+)' 60
    $code = $listed.Matches[0].Groups[1].Value
    $client = Start-ReviewProcess 'client' $exe @('--multiplayer-headless', "--multiplayer-lobby=$address",
        "--multiplayer-join-code=$code", '--multiplayer-name=SlotReviewClient', '--multiplayer-bot-slots',
        '--multiplayer-run-seconds=50')
    $children += $client
    (Wait-Receipt $client (Join-Path $root 'client.stderr.log') '\[slot-flow-multiplayer\]' 50).Line
    Write-Output $root
} finally {
    foreach ($child in $children) { if (!$child.HasExited) { $child.Kill() } }
    $env:VOXEL_SAVE_DIR = $oldSave
    $env:VOXEL_REVIEW_SLOT_FLOW = $oldReview
}
