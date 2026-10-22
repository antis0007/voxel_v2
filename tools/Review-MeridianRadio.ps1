param([string]$Root)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\MeridianReviewCommands.ps1"
$radioRepo=Split-Path $PSScriptRoot -Parent
if (!$Root) {$Root=Join-Path $radioRepo ('reviews\meridian-radio-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))}
if (Test-Path -LiteralPath $Root) {throw 'Use a fresh disposable review directory.'}
New-Item -ItemType Directory -Path $Root | Out-Null
$Root=(Resolve-Path -LiteralPath $Root).Path
$radioEnvironment=@{}
foreach($key in @('VOXEL_RECEIVER_REVIEW','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_REVIEW_PANEL')) {
    $radioEnvironment[$key]=[Environment]::GetEnvironmentVariable($key)
}
function Wait-Radio([string]$Pattern) {
    $deadline=[DateTime]::UtcNow.AddSeconds(40)
    do {
        if($script:radioProcess.HasExited){throw "Native radio review exited before $Pattern"}
        if((Test-Path -LiteralPath $script:radioLog) -and ((Get-Content -LiteralPath $script:radioLog -Raw) -match $Pattern)){return}
        Start-Sleep -Milliseconds 100
    } while([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Pattern"
}
function Step-Radio([string]$Id,[string]$Command,[string]$Pattern) {
    Write-MeridianCommand $Root "$Id $Command"
    Wait-Radio $Pattern
}
function Start-Radio([string]$Name,[bool]$Reload) {
    $script:radioLog="$Root\$Name.log"
    $radioArgs=@('--benchmark-world=superflat','--benchmark-seed=74','--benchmark-weather=clear',
        '--benchmark-view-chunks=2','--benchmark-width=1280','--benchmark-height=900','--benchmark-duration=3600')
    if($Reload){$radioArgs+='--benchmark-saved-world'}
    $script:radioProcess=Start-Process -FilePath "$radioRepo\target\release-fast\minecraft_style_voxel_prototype.exe" `
        -ArgumentList $radioArgs -WorkingDirectory $radioRepo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput $script:radioLog -RedirectStandardError "$Root\$Name.err.log"
}
try {
    $env:VOXEL_RECEIVER_REVIEW='1';$env:VOXEL_CAPTURE_DIR=$Root
    $env:VOXEL_SAVE_DIR=Join-Path $Root 'disposable-saves';$env:VOXEL_REVIEW_PANEL='8'
    Write-MeridianCommand $Root '00 report'
    Start-Radio native $false
    Wait-Radio '\[stream-profile\].*missing=0 .*horizon=true'
    Step-Radio 01 radio-supplies '01 radio-supplies level=0'
    Step-Radio 02 radio-guard 'guard_unchanged=true'
    Step-Radio 03 radio-upgrade '03 radio-upgrade level=1 knowledge=0'
    Step-Radio 04 radio-guard '04 radio-guard level=1 knowledge=0'
    Step-Radio 05 radio-listen '05 radio-listen level=1 knowledge=1'
    Step-Radio 06 radio-upgrade '06 radio-upgrade level=2 knowledge=1'
    Step-Radio 07 radio-guard '07 radio-guard level=2 knowledge=1'
    Step-Radio 08 radio-handshake '08 radio-handshake level=2 knowledge=3'
    Step-Radio 09 radio-upgrade '09 radio-upgrade level=3 knowledge=3'
    Step-Radio 10 radio-build '10 radio-build level=4 knowledge=3 dish=Some.*target=None'
    Step-Radio 11 radio-handshake '11 radio-handshake level=4 knowledge=3'
    Step-Radio 12 radio-aim '12 radio-aim level=4.*target=Some'
    Step-Radio 13 radio-handshake '13 radio-handshake level=4 knowledge=7'
    Step-Radio 13a radio-link-check '13a radio-link-check level=4'
    Step-Radio 14 radio-upgrade '14 radio-upgrade level=5 knowledge=7'
    Step-Radio 14a radio-link-check '14a radio-link-check level=5'
    Step-Radio 15 radio-guard '15 radio-guard level=5 knowledge=7'
    Wait-Radio '15 radio-guard stage=0 steel=58 circuits=62.*copper=64 crystal=62 iron=66'
    Step-Radio 15b radio-return 'uplink_ack=Meridian acknowledges your radio return'
    Step-Radio 16 save '16 save level=5 knowledge=7'
    Step-Radio 17 reload 'progression_reload_exact=true'
    Step-Radio 17a radio-break '17a radio-break level=5 knowledge=7'
    Step-Radio 17b radio-repair '17b radio-repair level=5 knowledge=7'
    Wait-Radio '17b radio-repair stage=0 steel=58 circuits=62.*copper=64 crystal=62 iron=64'
    Step-Radio 17c save '17c save level=5 knowledge=7'
    Step-Radio 17d reload '17d reload level=5 knowledge=7'
    Step-Radio 18 radio-look '18 radio-look level=5'
    Start-Sleep -Seconds 1
    Step-Radio 19-dish capture '19-dish\.png'
    Write-MeridianCommand $Root '20 show'
    Start-Sleep -Milliseconds 250
    Step-Radio 21-radio-ui capture '21-radio-ui\.png'
    Stop-Process -Id $script:radioProcess.Id;$script:radioProcess.WaitForExit()
    Write-MeridianCommand $Root '30 report'
    Start-Radio restarted $true
    Wait-Radio '30 report level=5 knowledge=7 dish=Some.*target=Some'
    Step-Radio 31 radio-guard 'guard_unchanged=true'
    $combined=(Get-Content "$Root\native.log" -Raw)+(Get-Content "$Root\restarted.log" -Raw)
    if($combined -match 'guard_unchanged=false|progression_reload_exact=false|weather_and_range=false|overhead_blocks_directional=false|reload_error|checkpoint_failed'){throw 'Radio acceptance failed.'}
    @('PASS paid levels 1 through 5 and exact total cost','PASS finite beacon and handshake knowledge',
      'PASS locked upgrades and free uplink rejected without mutation','PASS dish needs explicit aim before directional knowledge',
      'PASS physical dish deployment and paid console repair without losing research',
      'PASS storm attenuation, range limit and blocked overhead disables dish gain',
      'PASS a real radio alignment return accepted through the gated uplink',
      'PASS radio level, knowledge, anchor, aim and booking after checkpoint reload and process restart') |
        Set-Content -LiteralPath "$Root\result.txt"
    Get-Content -LiteralPath "$Root\result.txt"
}
finally {
    if($script:radioProcess -and !$script:radioProcess.HasExited){Stop-Process -Id $script:radioProcess.Id;$script:radioProcess.WaitForExit()}
    foreach($key in $radioEnvironment.Keys){[Environment]::SetEnvironmentVariable($key,$radioEnvironment[$key])}
}
