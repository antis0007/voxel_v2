param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [ValidateSet('overview','books','chest','lichen','far','fittings')][string]$View='overview',
    [ValidateSet('1','0.5','0.25')][string]$Scale='1'
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "target/furnishings-20261026/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$arguments=@('--benchmark-world=superflat','--benchmark-seed=424242',
    '--benchmark-generator-version=57',"--benchmark-scale=$Scale",'--benchmark-weather=clear',
    '--benchmark-review-time=0.5','--benchmark-sky-time=12','--benchmark-view-chunks=3',
    '--benchmark-width=1600','--benchmark-height=900','--benchmark-warmup=6',
    '--benchmark-terrain-camera=0,0,1,0,0','--benchmark-duration=3','--benchmark-capture-frames')
$vars=@{VOXEL_CAPTURE_DIR=$root;VOXEL_SAVE_DIR=(Join-Path $root 'disposable-save');
    VOXEL_REVIEW_ENEMIES='furnishings';VOXEL_ENEMY_REVIEW_SCENE='furnishings';VOXEL_FURNISHING_VIEW=$View;
    VOXEL_REVIEW_INPUT_ISOLATED='1';VOXEL_REVIEW_SKIP_CLOUDS='1';VOXEL_REVIEW_SKIP_HORIZON='1';
    VOXEL_REVIEW_DUNGEON=$null;VOXEL_REVIEW_WAND=$null;VOXEL_TERRAIN_ROUTE=$null;
    VOXEL_TERRAIN_EDITS=$null;VOXEL_TERRAIN_LIGHT=$null;VOXEL_TERRAIN_EXPOSURE=$null;
    VOXEL_REVIEW_TERRAIN=$null;VOXEL_REVIEW_WATER=$null;VOXEL_CAPTURE_VIDEO=$null}
$previous=@{};$process=$null
try {
    foreach($name in $vars.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name);[Environment]::SetEnvironmentVariable($name,$vars[$name])}
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;environment=$vars;
        scope='Native editable-block material swatches; rendering only; no performance or interaction acceptance'} |
        ConvertTo-Json -Depth 4 | Set-Content "$root/manifest.json"
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$process.Handle
    if(!$process.WaitForExit(120000)){$process.Kill();$process.WaitForExit();throw 'Capture exceeded 120 seconds'}
    if($process.ExitCode -ne 0 -or !(Test-Path "$root/camera.png")){throw "Capture failed: $root"}
    if(!(Select-String -LiteralPath "$root/stdout.log" -SimpleMatch '[furnishings-review]' -Quiet)){throw 'Missing scene receipt'}
    Write-Output $root
} finally {
    if($process -and !$process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
