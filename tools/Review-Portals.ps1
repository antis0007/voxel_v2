param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [ValidateSet('front','grazing','close','threshold','back','far','garden','underworld','manyfold-entry','manyfold-exit')][string]$View='front',
    [ValidateSet('1','0')][string]$Portals='1',
    [ValidateSet('1','0.5','0.25')][string]$Scale='1',
    [string]$Time='0.5',
    [string]$Veil='1',
    [switch]$Verify,
    [int]$MaxWallSeconds=120
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "target/portals-20261029/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$arguments=@('--benchmark-world=superflat','--benchmark-seed=424242',
    '--benchmark-generator-version=57',"--benchmark-scale=$Scale",'--benchmark-weather=clear',
    "--benchmark-review-time=$Time",'--benchmark-sky-time=12','--benchmark-view-chunks=4',
    '--benchmark-width=1600','--benchmark-height=900','--benchmark-warmup=6',
    '--benchmark-terrain-camera=0,0,1,0,0','--benchmark-duration=3','--benchmark-capture-frames')
# The surface hell gate of seed 424242 (generator 85): centre (218, -59), floor 8 m.
$underworld=$View -eq 'underworld'
$manyfold=$View -like 'manyfold-*'
if ($underworld -and $Verify) { throw 'Use the disposable realm fixture with --realm-review-roundtrip to verify Hell crossings.' }
if($underworld){$arguments=@('--benchmark-world=hills','--benchmark-seed=424242','--benchmark-generator-version=85',
    "--benchmark-scale=$Scale",'--benchmark-weather=clear',"--benchmark-review-time=$Time",'--benchmark-sky-time=12',
    '--benchmark-view-chunks=4','--benchmark-width=1600','--benchmark-height=900','--benchmark-warmup=8',
    '--benchmark-terrain-camera=218,-53.5,-509.6,0,0.04','--benchmark-duration=3','--benchmark-capture-frames')}
if($manyfold){$arguments=@('--benchmark-world=manyfold','--benchmark-seed=424242','--benchmark-generator-version=244',
    "--benchmark-scale=$Scale",'--benchmark-weather=clear',"--benchmark-review-time=$Time",
    '--benchmark-view-chunks=4','--benchmark-width=1600','--benchmark-height=900','--benchmark-warmup=8',
    '--benchmark-terrain-camera=0,0,1,0,0','--benchmark-duration=8','--benchmark-capture-frames')}
$vars=@{VOXEL_CAPTURE_DIR=$root;VOXEL_SAVE_DIR=(Join-Path $root 'disposable-save');
    VOXEL_REVIEW_ENEMIES=$(if($underworld){$null}else{'portals'});VOXEL_ENEMY_REVIEW_SCENE=$(if($underworld){$null}else{'portals'});VOXEL_PORTAL_VIEW=$View;
    VOXEL_PORTALS=$Portals;VOXEL_PORTAL_VEIL=$Veil;VOXEL_REVIEW_INPUT_ISOLATED='1';VOXEL_REVIEW_SKIP_CLOUDS='1';VOXEL_REVIEW_SKIP_HORIZON='1';
    VOXEL_FURNISHING_VIEW=$null;VOXEL_REVIEW_DUNGEON=$(if($manyfold){'manyfold:shell-' + $View.Substring(9)}else{$null});
    VOXEL_PORTAL_VERIFY=$(if($Verify){'1'}else{$null});VOXEL_REVIEW_WAND=$null;VOXEL_TERRAIN_ROUTE=$null;
    VOXEL_TERRAIN_EDITS=$null;VOXEL_TERRAIN_LIGHT=$null;VOXEL_TERRAIN_EXPOSURE=$null;
    VOXEL_REVIEW_TERRAIN=$null;VOXEL_REVIEW_WATER=$null;VOXEL_CAPTURE_VIDEO=$null}
$previous=@{};$process=$null
try {
    foreach($name in $vars.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name);[Environment]::SetEnvironmentVariable($name,$vars[$name])}
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;environment=$vars;
        scope=$(if($Verify){'Native portal capture and scripted crossing probe'}else{'Portal window and veil capture'})} |
        ConvertTo-Json -Depth 4 | Set-Content "$root/manifest.json"
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$process.Handle
    if(!$process.WaitForExit($MaxWallSeconds * 1000)){$process.Kill();$process.WaitForExit();throw "Capture exceeded $MaxWallSeconds seconds"}
    if($process.ExitCode -ne 0 -or !(Test-Path "$root/camera.png")){throw "Capture failed: $root"}
    if(!$underworld -and !$manyfold -and !(Select-String -LiteralPath "$root/stdout.log" -SimpleMatch '[portal-review]' -Quiet)){throw 'Missing scene receipt'}
    if($manyfold -and !(Select-String -LiteralPath "$root/stdout.log" -SimpleMatch '[dungeon-review]' -Quiet)){throw 'Missing Manyfold camera receipt'}
    if($Verify -and !(Select-String -LiteralPath "$root/stdout.log" -Pattern '^\[portal-verify\].*(blocked=true clear=true|crossed=true)' -Quiet)){throw 'Missing successful crossing receipt'}
    Write-Output $root
} finally {
    if($process -and !$process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
