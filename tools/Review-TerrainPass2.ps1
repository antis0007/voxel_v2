param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [ValidateSet('fixture','meadow','motion','edits','light','night','perf')][string]$Case='fixture',
    [int]$Width=1280, [int]$Height=720, [int]$Seconds=32,
    [string]$DebugView='', [string]$Light='', [string]$Wind='',
    [string]$Exposure='', [string]$VoxelScale='1m', [double]$Time=0.5,
    [switch]$Animate, [switch]$Video
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "reviews/terrain-pass2-20261021/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$argsList=@('--benchmark-world=hills','--benchmark-seed=2738958700','--benchmark-weather=partly',
    '--benchmark-view-chunks=4',"--benchmark-width=$Width","--benchmark-height=$Height",
    '--benchmark-terrain-camera=0,0,5,0,-0.20','--benchmark-warmup=8',"--benchmark-duration=$Seconds",
    "--benchmark-review-time=$Time","--benchmark-scale=$VoxelScale")
if(-not $Animate){$argsList+='--benchmark-sky-time=12'}
if($Case -ne 'perf'){$argsList+='--benchmark-capture-frames'}
if($Video){$argsList+='--benchmark-capture-interval=0.1'}
if($DebugView){$argsList+="--benchmark-debug=$DebugView"}
$vars=@{VOXEL_CAPTURE_DIR=$root; VOXEL_REVIEW_TERRAIN=$null; VOXEL_REVIEW_WAND=$null;
    VOXEL_TERRAIN_ROUTE=$null; VOXEL_TERRAIN_EDITS=$null; VOXEL_TERRAIN_EXPOSURE=$Exposure;
    VOXEL_TERRAIN_LIGHT=$Light; VOXEL_TERRAIN_WIND=$Wind}
if($Case -in @('fixture','light','night','perf')){$vars.VOXEL_REVIEW_TERRAIN='10'}
if($Case -eq 'motion'){$vars.VOXEL_TERRAIN_ROUTE='1'}
if($Case -eq 'motion'){$argsList+='--benchmark-terrain-camera=0,0,2,0,-0.20'}
if($Case -eq 'edits'){$vars.VOXEL_TERRAIN_EDITS='1'}
$previous=@{}
foreach($name in $vars.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name);[Environment]::SetEnvironmentVariable($name,$vars[$name])}
try {
    [ordered]@{utc=[DateTime]::UtcNow.ToString('o');executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;
        arguments=$argsList;environment=$vars;fov_vertical_degrees=70;render_scale=1;
        capture_note='Readback adds frame cost. Use perf case without captures for timing.'} |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$root/manifest.json"
    $p=Start-Process -FilePath $exe -ArgumentList $argsList -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$p.Handle
    while(-not $p.WaitForExit(500)){}
    [ordered]@{exit_code=$p.ExitCode} | ConvertTo-Json | Set-Content "$root/process.json"
    if($p.ExitCode -ne 0){throw "Renderer exit $($p.ExitCode): $root"}
    Select-String -LiteralPath "$root/stdout.log" -Pattern '^\[benchmark\]' | ForEach-Object {$_.Line} | Set-Content "$root/summary.txt"
    if(-not(Test-Path "$root/summary.txt")){throw 'Missing benchmark summary'}
    if($Video){
        $frames=@(Get-ChildItem -LiteralPath $root -Filter 'bench-*.png' | Sort-Object Name)
        $concat=[Collections.Generic.List[string]]::new()
        for($i=0;$i -lt $frames.Count;$i++){
            $concat.Add("file '$($frames[$i].Name)'")
            $duration=0.1
            if($i+1 -lt $frames.Count){$duration=([int]($frames[$i+1].BaseName -replace 'bench-','')-[int]($frames[$i].BaseName -replace 'bench-',''))*0.1}
            $concat.Add('duration '+$duration.ToString('0.###',[Globalization.CultureInfo]::InvariantCulture))
        }
        $concat | Set-Content -Encoding ascii "$root/frames.txt"
        & ffmpeg -hide_banner -loglevel error -f concat -safe 0 -i "$root/frames.txt" -fps_mode vfr -c:v libx264 -crf 18 -pix_fmt yuv420p "$root/traversal.mp4"
        if($LASTEXITCODE -ne 0){throw 'Video encoding failed'}
    }
    Write-Output $root
    Get-Content "$root/summary.txt"
} finally {foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
