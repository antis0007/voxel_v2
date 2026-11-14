# Native capture of a creature lineup by bestiary name or kind number.
# Works for authored kinds and for species the Axiom panel or an agent created
# (kinds from 1000), so a new row gets art evidence before anyone plays it.
#   .\tools\Review-Creatures.ps1 -Executable target\release-fast\minecraft_style_voxel_prototype.exe `
#       -Label boar-before -Creatures 'Cow,Boar,Deer,Marsh Fowl'
# -Scene flora stages a ripe field with a crop harvester; -Ripe leaves the harvester out.
param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [string]$Creatures='',
    [ValidateSet('fauna','bestiary','flora')][string]$Scene='fauna',
    [switch]$Ripe,
    [ValidateSet('1','0.5','0.25')][string]$Scale='1',
    [ValidateSet('combat','poses','materials')][string]$Mode='poses',
    [double]$PoseTime=-1,
    [double]$Duration=3,
    [double]$CaptureInterval=0.25
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "target/creature-review/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$arguments=@('--benchmark-world=superflat','--benchmark-seed=424242',
    '--benchmark-generator-version=57',"--benchmark-scale=$Scale",'--benchmark-weather=clear',
    '--benchmark-review-time=0.5','--benchmark-sky-time=12','--benchmark-view-chunks=3',
    '--benchmark-width=1600','--benchmark-height=900','--benchmark-warmup=6',
    '--benchmark-terrain-camera=0,0,1,0,0',"--benchmark-duration=$Duration",
    "--benchmark-capture-interval=$CaptureInterval",'--benchmark-capture-frames')
$vars=@{VOXEL_CAPTURE_DIR=$root;VOXEL_SAVE_DIR=(Join-Path $root 'disposable-save');
    VOXEL_REVIEW_ENEMIES=$Mode;VOXEL_ENEMY_REVIEW_SCENE=$Scene;
    VOXEL_ENEMY_REVIEW_KINDS=$(if($Creatures){$Creatures}else{$null});
    VOXEL_ENEMY_REVIEW_POSE_TIME=$(if($PoseTime -ge 0){$PoseTime.ToString([Globalization.CultureInfo]::InvariantCulture)}else{$null});
    VOXEL_FLORA_REVIEW=$(if($Ripe){'ripe'}else{$null});
    VOXEL_REVIEW_INPUT_ISOLATED='1';VOXEL_REVIEW_SKIP_CLOUDS='1';VOXEL_REVIEW_SKIP_HORIZON='1';
    VOXEL_ENEMY_REVIEW_KIND=$null;VOXEL_REVIEW_DUNGEON=$null;VOXEL_REVIEW_WAND=$null;
    VOXEL_TERRAIN_ROUTE=$null;VOXEL_TERRAIN_EDITS=$null;VOXEL_REVIEW_TERRAIN=$null;
    VOXEL_REVIEW_WATER=$null;VOXEL_CAPTURE_VIDEO=$null}
$previous=@{};$process=$null
try {
    foreach($name in $vars.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name);[Environment]::SetEnvironmentVariable($name,$vars[$name])}
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;environment=$vars;
        scope='Native creature art capture. No performance or gameplay acceptance.'} |
        ConvertTo-Json -Depth 4 | Set-Content "$root/manifest.json"
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$process.Handle
    if(!$process.WaitForExit(240000)){$process.Kill();$process.WaitForExit();throw 'Capture exceeded 240 seconds'}
    if($process.ExitCode -ne 0 -or !(Test-Path "$root/camera.png")){throw "Capture failed: $root"}
    Write-Output $root
} finally {
    if($process -and !$process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
