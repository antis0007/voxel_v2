# Native visual review only. Records a production pose, then sweeps orbital time
# around it with a fixed camera. Uses disposable saves and no gameplay physics.
param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Root,
    [ValidateSet('mirror','habitat','foundry','skyhook')][string]$Structure='mirror',
    [uint32]$Seed=74,
    [switch]$Detail,
    [double]$OrbitalSeconds=30,
    [int]$Seconds=12
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/MeridianReviewCommands.ps1"
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
if(Test-Path -LiteralPath $Root){throw 'Use a fresh review directory.'}
$Root=[IO.Path]::GetFullPath($Root)
New-Item -ItemType Directory -Path $Root | Out-Null
$candidate=Join-Path $Root 'candidate.exe'
Copy-Item -LiteralPath $exe -Destination $candidate
$culture=[Globalization.CultureInfo]::InvariantCulture
function Number([double]$value){$value.ToString('R',$culture)}
$previous=@{}
$process=$null
$common=@('--benchmark-world=hills',"--benchmark-seed=$Seed",'--benchmark-scale=1',
    '--benchmark-generator-version=25','--benchmark-weather=clear','--benchmark-no-indirect',
    '--benchmark-view-chunks=3','--benchmark-width=1280','--benchmark-height=900')
function Wait-Line([string]$pattern){
    $deadline=[DateTime]::UtcNow.AddSeconds(60)
    do {
        if($process.HasExited){throw 'Pose review exited before recording its camera.'}
        if((Test-Path "$poseRoot/native.log") -and
            (Get-Content "$poseRoot/native.log" -Raw) -match $pattern){return}
        Start-Sleep -Milliseconds 100
    }while([DateTime]::UtcNow -lt $deadline)
    throw "Pose review timed out: $pattern"
}
try {
    foreach($key in @('VOXEL_RECEIVER_REVIEW','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR',
        'VOXEL_REVIEW_NO_ORBITAL','VOXEL_REVIEW_DUNGEON','VOXEL_TERRAIN_ROUTE')){
        $previous[$key]=[Environment]::GetEnvironmentVariable($key)
        [Environment]::SetEnvironmentVariable($key,$null)
    }
    $poseRoot=Join-Path $Root 'pose'
    New-Item -ItemType Directory -Path $poseRoot | Out-Null
    $env:VOXEL_RECEIVER_REVIEW='1'
    $env:VOXEL_CAPTURE_DIR=$poseRoot
    $env:VOXEL_SAVE_DIR=Join-Path $poseRoot 'disposable-saves'
    Write-MeridianCommand $poseRoot '00 report'
    $process=Start-Process -FilePath $candidate -ArgumentList ($common+@('--benchmark-duration=3600')) `
        -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$poseRoot/native.log" -RedirectStandardError "$poseRoot/native.err.log"
    Wait-Line '00 report stage=0'
    Write-MeridianCommand $poseRoot 'clock megastructure-day'
    Wait-Line 'clock megastructure-day stage=0'
    $command=if($Structure -eq 'skyhook'){'skyhook-detail'}else{"megastructure-$Structure"}
    if($Detail -and $Structure -ne 'skyhook'){$command+='-detail'}
    Write-MeridianCommand $poseRoot "pose $command"
    Wait-Line '\[orbital-pose\]'
    $poseLine=(Select-String -Path "$poseRoot/native.log" -Pattern '\[orbital-pose\]' | Select-Object -Last 1).Line
    if($poseLine -notmatch 'days=([\d.]+) position=([-\d.]+),([-\d.]+),([-\d.]+) yaw=([-\d.]+) pitch=([-\d.]+)'){
        throw 'Invalid recorded orbital pose.'
    }
    $poseValues=1..6 | ForEach-Object {[double]::Parse($Matches[$_],$culture)}
    $process.Kill();$process.WaitForExit();$process=$null
    $sweepRoot=Join-Path $Root 'sweep'
    New-Item -ItemType Directory -Path $sweepRoot | Out-Null
    [Environment]::SetEnvironmentVariable('VOXEL_RECEIVER_REVIEW',$null)
    $env:VOXEL_CAPTURE_DIR=$sweepRoot
    $env:VOXEL_SAVE_DIR=Join-Path $sweepRoot 'disposable-saves'
    $camera=@($poseValues[1],$poseValues[3],-($poseValues[2]+500),$poseValues[4],$poseValues[5]) |
        ForEach-Object {Number $_}
    $span=[Math]::Clamp($OrbitalSeconds,0,120)/86400.0
    $arguments=$common+@('--benchmark-warmup=6',"--benchmark-duration=$Seconds",
        "--benchmark-review-time=$(Number ($poseValues[0]%1))",
        "--benchmark-orbital-time=$(Number ($poseValues[0]-$span/2))",
        "--benchmark-orbital-time-end=$(Number ($poseValues[0]+$span/2))",
        "--benchmark-terrain-camera=$($camera -join ',')",
        '--benchmark-capture-frames','--benchmark-capture-interval=2')
    [ordered]@{executable=$candidate;sha256=(Get-FileHash $candidate).Hash;structure=$Structure;detail=[bool]($Detail -or $Structure -eq 'skyhook');
        pose=$poseLine;arguments=$arguments;scope='Native orbital sweep; physics disabled; disposable saves'} |
        ConvertTo-Json -Depth 4 | Set-Content "$Root/manifest.json"
    $process=Start-Process -FilePath $candidate -ArgumentList $arguments -WorkingDirectory $repo `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput "$sweepRoot/native.log" `
        -RedirectStandardError "$sweepRoot/native.err.log"
    if(-not $process.WaitForExit(120000)){throw 'Orbital sweep exceeded its wall-time bound.'}
    if($process.ExitCode -ne 0){throw "Orbital sweep exited $($process.ExitCode)."}
    if(-not(Test-Path "$sweepRoot/camera.png")){throw 'No fully resident final capture was produced.'}
    Write-Output $Root
} finally {
    if($process -and -not $process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($key in $previous.Keys){[Environment]::SetEnvironmentVariable($key,$previous[$key])}
}
