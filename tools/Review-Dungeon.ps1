param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [ValidateSet('1','0.5','0.25')][string]$Scale='1',
    [ValidateSet('entry','reverse','lower','roof','motion','cosmos')][string]$Pose='entry',
    [int]$GeneratorVersion=0,
    [int]$Seconds=32,
    [int]$MaxWallSeconds=180,
    [switch]$Video
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "target/dungeon-review/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$metersPerVoxel=[double]::Parse($Scale,[Globalization.CultureInfo]::InvariantCulture)
$viewChunks=[Math]::Max(4,[int][Math]::Ceiling(32.0/(32.0*$metersPerVoxel)))
$arguments=@('--benchmark-world=hills','--benchmark-seed=2738958700',
    "--benchmark-scale=$Scale",'--benchmark-weather=clear','--benchmark-review-time=0.5',
    "--benchmark-view-chunks=$viewChunks",'--benchmark-width=2560','--benchmark-height=1440',
    '--benchmark-terrain-camera=0,0,1,0,0','--benchmark-warmup=14',
    "--benchmark-duration=$Seconds",'--benchmark-capture-frames')
if($Video){$arguments+='--benchmark-capture-interval=2'}
if($GeneratorVersion -gt 0){$arguments+="--benchmark-generator-version=$GeneratorVersion"}
$vars=@{VOXEL_REVIEW_DUNGEON=$Pose;VOXEL_CAPTURE_DIR=$root;
    VOXEL_SAVE_DIR=(Join-Path $root 'generated-world');VOXEL_REVIEW_WAND=$null;
    VOXEL_TERRAIN_ROUTE=$null;VOXEL_TERRAIN_EDITS=$null;VOXEL_TERRAIN_LIGHT=$null;
    VOXEL_TERRAIN_EXPOSURE=$null;VOXEL_REVIEW_ENEMIES=$null;VOXEL_HART_ORBIT=$null;
    VOXEL_REVIEW_SKIP_CLOUDS=$null;VOXEL_REVIEW_SKIP_HORIZON=$null;
    VOXEL_HORIZON_ROUTE=$null;VOXEL_HORIZON_EDITS=$null;VOXEL_HORIZON_SOURCE=$null;
    VOXEL_CAPTURE_VIDEO=$null;VOXEL_RENDER_TIMING=$null}
$previous=@{}
$process=$null
try {
    foreach($name in $vars.Keys){
        $previous[$name]=[Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name,$vars[$name])
    }
    [ordered]@{utc=[DateTime]::UtcNow.ToString('o');executable=$exe;
        sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;environment=$vars;
        scope='Generated hall render review; camera motion does not prove traversal; capture readback excludes performance acceptance'} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath "$root/manifest.json"
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$process.Handle
    $process.Id | Set-Content -LiteralPath "$root/pid.txt"
    if(-not $process.WaitForExit($MaxWallSeconds*1000)){
        $process.Kill();$process.WaitForExit()
        throw "Dungeon review exceeded ${MaxWallSeconds}s wall limit: $root"
    }
    [ordered]@{pid=$process.Id;exit_code=$process.ExitCode;
        started_utc=$process.StartTime.ToUniversalTime().ToString('o');
        ended_utc=$process.ExitTime.ToUniversalTime().ToString('o')} |
        ConvertTo-Json | Set-Content -LiteralPath "$root/process.json"
    if($process.ExitCode -ne 0){throw "Dungeon review exited $($process.ExitCode): $root"}
    $receipt=Select-String -LiteralPath "$root/stdout.log" -Pattern '^\[dungeon-review\]'
    if(-not $receipt){throw "Missing generated-hall camera receipt: $root"}
    if(-not(Test-Path -LiteralPath "$root/camera.png")){throw "Hall never reached resident capture: $root"}
    $receipt.Line
    Write-Output $root
} finally {
    if($process -and -not $process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
