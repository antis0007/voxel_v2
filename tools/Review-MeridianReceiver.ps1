param([string]$Root)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\MeridianReviewCommands.ps1"
$receiverRepo = Split-Path $PSScriptRoot -Parent
if (!$Root) { $Root = Join-Path $receiverRepo ('reviews\meridian-acceptance-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')) }
if (Test-Path -LiteralPath $Root) { throw 'Use a fresh disposable review directory.' }
New-Item -ItemType Directory -Path $Root | Out-Null
$Root = (Resolve-Path -LiteralPath $Root).Path
$receiverExe = Join-Path $receiverRepo 'target\release-fast\minecraft_style_voxel_prototype.exe'
$receiverEnvironment = @{}
foreach ($key in @('VOXEL_RECEIVER_REVIEW','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_REVIEW_PANEL','VOXEL_REVIEW_NO_ORBITAL')) {
    $receiverEnvironment[$key] = [Environment]::GetEnvironmentVariable($key)
}
$env:VOXEL_RECEIVER_REVIEW = '1'
$env:VOXEL_CAPTURE_DIR = $Root
$env:VOXEL_SAVE_DIR = Join-Path $Root 'disposable-saves'
$env:VOXEL_REVIEW_PANEL = '8'
Remove-Item Env:VOXEL_REVIEW_NO_ORBITAL -ErrorAction SilentlyContinue
$receiverArgs = @('--benchmark-world=superflat','--benchmark-generator-version=25','--benchmark-seed=74',
 '--benchmark-weather=clear','--benchmark-no-indirect','--benchmark-view-chunks=2',
 '--benchmark-width=1280','--benchmark-height=900','--benchmark-duration=3600','--benchmark-capture-hud')
function Read-ReceiverLog([string]$Path) {
    $stream = [IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
    $reader = [IO.StreamReader]::new($stream)
    try { return $reader.ReadToEnd() } finally { $reader.Dispose() }
}
function Wait-Receiver([string]$Pattern, [int]$Seconds = 25) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        if ((Test-Path -LiteralPath $script:receiverLog) -and
            [regex]::IsMatch((Read-ReceiverLog $script:receiverLog),$Pattern)) { return }
        if ($script:receiverScenarioProcess.HasExited) { throw "Native review exited before: $Pattern" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Native review timed out waiting for: $Pattern"
}
function Step-Receiver([string]$Id,[string]$Command,[string]$Pattern) {
    Write-MeridianCommand $Root "$Id $Command"
    Wait-Receiver $Pattern
}
function Capture-Receiver([string]$Id) {
    Write-MeridianCommand $Root "$Id capture"
    Wait-Receiver ([regex]::Escape("$Id.png"))
}
function Start-Receiver([string]$Name,[bool]$Reload) {
    $script:receiverLog = Join-Path $Root "$Name.log"
    $argsNow = $receiverArgs
    if ($Reload) { $argsNow += '--benchmark-saved-world' }
    $script:receiverScenarioProcess = Start-Process -FilePath $receiverExe -ArgumentList $argsNow -WorkingDirectory $receiverRepo `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput $script:receiverLog -RedirectStandardError (Join-Path $Root "$Name.err.log")
}
try {
    Write-MeridianCommand $Root '00 report'
    Start-Receiver native $false
    Wait-Receiver '00 report stage=0 steel=0 circuits=0.*copper=16 crystal=2 iron=16'
    Wait-Receiver '\[stream-profile\].*missing=0 .*horizon=true'
    Step-Receiver 01 deploy '01 deploy stage=0 steel=0 circuits=0 anchor=Some.*copper=12 crystal=2 iron=8'
    Step-Receiver 01a focus 'grounded_receiver_focus=true'
    Capture-Receiver 02-deployed
    Step-Receiver 03 order '03 order stage=1 steel=0 circuits=0.*copper=8 crystal=1 iron=8'
    Step-Receiver 04 flight '04 flight stage=1'
    Start-Sleep -Seconds 3
    Capture-Receiver 05-capsule-descending
    Start-Sleep -Seconds 5
    Step-Receiver 06 look-console '06 look-console stage=1'
    Write-MeridianCommand $Root '06a hide'
    Start-Sleep -Milliseconds 250
    Capture-Receiver 07-arrived-physical
    Write-MeridianCommand $Root '07a show'
    Start-Sleep -Milliseconds 250
    Capture-Receiver 07-arrived
    Step-Receiver 08 full-partial '08 full-partial stage=1 steel=60 circuits=0'
    Step-Receiver 09 collect '09 collect stage=1 steel=60 circuits=0'
    Step-Receiver 10 check-full 'full_inventory_atomic=true stage=1'
    Step-Receiver 11 full-second '11 full-second stage=1 steel=56 circuits=0'
    Step-Receiver 12 collect '12 collect stage=1 steel=56 circuits=0'
    # Unique report proves the second check ran, rather than matching the first.
    Step-Receiver 13 check-full '13 check-full stage=1 steel=56 circuits=0'
    $nativeText = Read-ReceiverLog $script:receiverLog
    if ([regex]::Matches($nativeText,'full_inventory_atomic=true').Count -ne 2) { throw 'Both full-pack checks must pass.' }
    Capture-Receiver 14-full-pack-retained
    Step-Receiver 15 free '15 free stage=1 steel=0 circuits=0.*copper=8 crystal=1 iron=8'
    Step-Receiver 16 collect '16 collect stage=2 steel=8 circuits=4.*copper=8 crystal=1 iron=8'
    Capture-Receiver 17-completed
    Step-Receiver 18 save '18 save stage=2 steel=8 circuits=4'
    Step-Receiver 19 reload 'reload_exact=true version=\d+'
    Step-Receiver 20 duplicates 'duplicate_requests_rejected=true'
    Step-Receiver 21 restore 'restore_preserves_completion=true'
    Step-Receiver 22 save '22 save stage=2 steel=8 circuits=4.*iron=6'
    Step-Receiver 23 reload '23 reload stage=2 steel=8 circuits=4.*iron=6'
    if ([regex]::Matches((Read-ReceiverLog $script:receiverLog),'\[receiver-review\] reload_exact=true').Count -ne 2) { throw 'Both completed checkpoints must reload exactly.' }
    Capture-Receiver 24-restored-reloaded
    Stop-Process -Id $script:receiverScenarioProcess.Id
    $script:receiverScenarioProcess.WaitForExit()
    Write-MeridianCommand $Root '30 report'
    Start-Receiver restarted $true
    Wait-Receiver '30 report stage=2 steel=8 circuits=4.*copper=8 crystal=1 iron=6'
    Wait-Receiver '\[stream-profile\].*missing=0 .*horizon=true'
    Step-Receiver 31 duplicates 'duplicate_requests_rejected=true'
    Capture-Receiver 32-process-restart
    $all = (Read-ReceiverLog (Join-Path $Root native.log)) + (Read-ReceiverLog (Join-Path $Root restarted.log))
    if ($all -match '(?:atomic|exact|rejected|completion)=false|reload_error|checkpoint_failed') { throw 'Native acceptance reported a failure.' }
    @('PASS deployment and exact payment','PASS grounded interaction ray reaches receiver plinth','PASS capsule descent and arrival','PASS partial-first-item rollback',
      'PASS second-item-full rollback','PASS one complete cargo collection','PASS completed save/reload twice',
      'PASS console restoration retains completion','PASS process restart retains completion',
      'PASS duplicate order, deployment and collection requests rejected after reload and restart') |
        Set-Content -LiteralPath (Join-Path $Root result.txt)
    Get-Content -LiteralPath (Join-Path $Root result.txt)
    Write-Output "Evidence: $Root"
} finally {
    if ($script:receiverScenarioProcess -and !$script:receiverScenarioProcess.HasExited) {
        Stop-Process -Id $script:receiverScenarioProcess.Id
    }
    foreach ($key in $receiverEnvironment.Keys) { [Environment]::SetEnvironmentVariable($key,$receiverEnvironment[$key]) }
}
