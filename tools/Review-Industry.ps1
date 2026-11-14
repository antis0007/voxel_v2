param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [switch]$Clean
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "artifacts/industry-native/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$arguments=@('--benchmark-world=superflat','--benchmark-seed=424242',
    '--benchmark-generator-version=57','--benchmark-scale=1','--benchmark-weather=clear',
    '--benchmark-review-time=0.5','--benchmark-sky-time=12','--benchmark-view-chunks=3',
    '--benchmark-width=1600','--benchmark-height=900','--benchmark-warmup=10',
    '--benchmark-terrain-camera=0,0,1,0,0','--benchmark-duration=4','--benchmark-capture-frames')
$vars=@{VOXEL_CAPTURE_DIR=$root;VOXEL_SAVE_DIR=(Join-Path $root 'disposable-save');
    VOXEL_REVIEW_ENEMIES='industry';VOXEL_ENEMY_REVIEW_SCENE='industry';
    VOXEL_REVIEW_INPUT_ISOLATED='1';VOXEL_REVIEW_SKIP_CLOUDS='1';VOXEL_REVIEW_SKIP_HORIZON='1';
    VOXEL_INDUSTRY_REVIEW_CLEAN=$(if($Clean){'1'}else{$null});
    VOXEL_REVIEW_DUNGEON=$null;VOXEL_REVIEW_WAND=$null;VOXEL_TERRAIN_ROUTE=$null;
    VOXEL_TERRAIN_EDITS=$null;VOXEL_REVIEW_TERRAIN=$null;VOXEL_REVIEW_WATER=$null;
    VOXEL_CAPTURE_VIDEO=$null}
$previous=@{};$process=$null
try {
    foreach($name in $vars.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name);[Environment]::SetEnvironmentVariable($name,$vars[$name])}
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;clean=$Clean.IsPresent;
        scope='Disposable native industry rendering; no broad performance or release acceptance'} |
        ConvertTo-Json -Depth 4 | Set-Content "$root/manifest.json"
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$process.Handle
    Write-Output "Industry capture PID $($process.Id): $root"
    while(!$process.HasExited){
        if($process.WaitForExit(1000)){break}
        if(((Get-Date)-$process.StartTime).TotalSeconds -gt 180){$process.Kill();throw 'Industry capture exceeded 180 seconds'}
    }
    $process.WaitForExit()
    $process.ExitCode | Set-Content "$root/exit-code.txt"
    if(!(Test-Path "$root/camera.png") -and (Test-Path "$root/bench-000.png")) {
        Copy-Item -LiteralPath "$root/bench-000.png" -Destination "$root/camera.png"
    }
    if($process.ExitCode -ne 0 -or !(Test-Path "$root/camera.png")){throw "Capture failed: $root"}
    if(!(Select-String -LiteralPath "$root/stdout.log" -SimpleMatch '[industry-review]' -Quiet)){throw 'Missing industry scene receipt'}
    Write-Output $root
} finally {
    if($process -and !$process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
