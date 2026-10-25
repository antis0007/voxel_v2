param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [ValidateSet('idle','motion','edits','handoff')][string]$Case='idle',
    [int]$Width=1280, [int]$Height=720, [int]$Seconds=32,
    [string]$Camera='44,35,1,-1.0,-0.12',
    [int]$CloudDivisor=3,
    [ValidateSet('clear','partly','overcast','storm','winter')][string]$Weather='partly',
    [ValidateSet('','preview','canonical')][string]$HorizonSource='',
    [string]$DebugView='', [switch]$Capture, [switch]$Video, [switch]$CaptureHud,
    [switch]$NoShadows, [switch]$Physics, [switch]$SkipClouds, [switch]$SkipHorizon,
    [switch]$GpuTelemetry, [switch]$FrameTiming, [switch]$NormalPresent,
    [int]$Warmup=12,
    [int]$GeneratorVersion=0,
    [string]$ReviewWand='2', [string]$TerrainExposure='', [string]$TerrainWind='',
    [string]$VoxelScale=''
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "target/reviews/forest-repair-20261021/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
[ordered]@{accepted_for_performance=$false;reason='Run awaits scope, process and timing review'} |
    ConvertTo-Json | Set-Content -LiteralPath "$root/acceptance.json"
$argsList=@('--benchmark-world=hills','--benchmark-seed=2738958700',"--benchmark-weather=$Weather",
    '--benchmark-review-time=0.5','--benchmark-sky-time=12','--benchmark-view-chunks=6',
    "--benchmark-cloud-divisor=$CloudDivisor","--benchmark-width=$Width","--benchmark-height=$Height",
    "--benchmark-terrain-camera=$Camera","--benchmark-warmup=$Warmup","--benchmark-duration=$Seconds")
if($GeneratorVersion -gt 0){$argsList+="--benchmark-generator-version=$GeneratorVersion"}
if($VoxelScale){$argsList+="--benchmark-scale=$VoxelScale"}
if($Capture -or $Video){$argsList+='--benchmark-capture-frames'}
if($CaptureHud){$argsList+='--benchmark-capture-hud'}
if($Video){$argsList+='--benchmark-capture-interval=0.5'}
if($DebugView){$argsList+="--benchmark-debug=$DebugView"}
if($NoShadows){$argsList+='--benchmark-no-shadows'}
if($Physics){$argsList+='--benchmark-physics'}
if($Case -eq 'handoff'){$argsList+='--benchmark-horizon-sweep'}
$vars=@{VOXEL_CAPTURE_DIR=$root;VOXEL_REVIEW_WAND='2';VOXEL_TERRAIN_ROUTE=$null;
    VOXEL_TERRAIN_EDITS=$null;VOXEL_TERRAIN_EXPOSURE=$null;VOXEL_TERRAIN_LIGHT=$null;
    VOXEL_TERRAIN_WIND=$null;VOXEL_REVIEW_TERRAIN=$null;VOXEL_REVIEW_WATER=$null}
$vars.VOXEL_REVIEW_WAND=if($ReviewWand){$ReviewWand}else{$null}
$vars.VOXEL_TERRAIN_EXPOSURE=if($TerrainExposure){$TerrainExposure}else{$null}
$vars.VOXEL_TERRAIN_WIND=if($TerrainWind){$TerrainWind}else{$null}
$vars.VOXEL_SAVE_DIR=Join-Path $root 'generated-world'
$vars.VOXEL_CAPTURE_VIDEO=$null
$vars.VOXEL_CAPTURE_VIDEO_FPS=$null
$vars.VOXEL_REVIEW_SKIP_CLOUDS=if($SkipClouds){'1'}else{$null}
$vars.VOXEL_REVIEW_SKIP_HORIZON=if($SkipHorizon){'1'}else{$null}
$vars.VOXEL_HORIZON_SOURCE=$HorizonSource
$vars.VOXEL_HORIZON_ROUTE=$null
$vars.VOXEL_HORIZON_EDITS=$null
$vars.VOXEL_HORIZON_EXACT_PATCH=$null
$vars.VOXEL_HORIZON_REFERENCE_VOXELS=$null
$vars.VOXEL_HORIZON_DELAY_MS=$null
$vars.VOXEL_HORIZON_UPLOAD_BYTES=$null
$vars.VOXEL_HORIZON_TRACE=[Environment]::GetEnvironmentVariable('VOXEL_HORIZON_TRACE')
$vars.VOXEL_HORIZON_CULL=[Environment]::GetEnvironmentVariable('VOXEL_HORIZON_CULL')
$vars.VOXEL_RENDER_TIMING=if($FrameTiming){'1'}else{$null}
$vars.VOXEL_GRIMOIRE_REVIEW=[Environment]::GetEnvironmentVariable('VOXEL_GRIMOIRE_REVIEW')
$vars.VOXEL_REVIEW_INPUT_ISOLATED=[Environment]::GetEnvironmentVariable('VOXEL_REVIEW_INPUT_ISOLATED')
$vars.VOXEL_REVIEW_SPELL_CARD=[Environment]::GetEnvironmentVariable('VOXEL_REVIEW_SPELL_CARD')
$vars.VOXEL_REVIEW_CODEX=[Environment]::GetEnvironmentVariable('VOXEL_REVIEW_CODEX')
$vars.VOXEL_BENCHMARK_NORMAL_PRESENT=if($NormalPresent){'1'}else{$null}
if($Case -eq 'motion'){$vars.VOXEL_TERRAIN_ROUTE='1'}
if($Case -eq 'edits'){$vars.VOXEL_TERRAIN_EDITS='1'}
$previous=@{}
$telemetry=$null
function Set-ReviewEnvironment([string]$Name,$Value){
    if($null -eq $Value){
        Remove-Item -LiteralPath "Env:$Name" -ErrorAction SilentlyContinue
    }else{
        [Environment]::SetEnvironmentVariable($Name,[string]$Value)
    }
}
foreach($name in $vars.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name);Set-ReviewEnvironment $name $vars[$name]}
try {
    [ordered]@{utc=[DateTime]::UtcNow.ToString('o');executable=$exe;
        sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$argsList;environment=$vars;
        case=$Case;capture_readback=($Capture -or $Video);gpu_telemetry=[bool]$GpuTelemetry;profile='release-fast, optimized';
        note='Equivalent generated scene; original forest save/camera unavailable. Loading frames excluded. See logs for actual adapter, passes, effective cloud scale.'} |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$root/manifest.json"
    $p=Start-Process -FilePath $exe -ArgumentList $argsList -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$p.Handle
    $p.Id | Set-Content -LiteralPath "$root/pid.txt"
    if($GpuTelemetry){
        $smi=Join-Path $env:SystemRoot 'System32/nvidia-smi.exe'
        $telemetry=Start-Process -FilePath $smi -ArgumentList @(
            '--query-gpu=timestamp,pstate,clocks.current.graphics,clocks.current.memory,utilization.gpu,temperature.gpu,power.draw',
            '--format=csv','--loop=1') -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput "$root/gpu-telemetry.csv" -RedirectStandardError "$root/gpu-telemetry-error.log"
    }
    $peakWorking=0L; $peakPrivate=0L
    while(-not $p.WaitForExit(500)){
        $p.Refresh()
        $peakWorking=[Math]::Max($peakWorking,$p.WorkingSet64)
        $peakPrivate=[Math]::Max($peakPrivate,$p.PrivateMemorySize64)
    }
    [ordered]@{pid=$p.Id;started_utc=$p.StartTime.ToUniversalTime().ToString('o');
        ended_utc=$p.ExitTime.ToUniversalTime().ToString('o');exit_code=$p.ExitCode;
        peak_working_bytes=$peakWorking;peak_private_bytes=$peakPrivate} |
        ConvertTo-Json | Set-Content -LiteralPath "$root/process.json"
    $summary=@(Select-String -LiteralPath "$root/stdout.log" -Pattern '^\[benchmark\]' | ForEach-Object {$_.Line})
    $summary | Set-Content -LiteralPath "$root/summary.txt"
    if($p.ExitCode -ne 0 -or -not ($summary -match 'avg_fps=')){throw "Incomplete renderer run: $root"}
    if(-not ($summary -match 'terrain_draws=[1-9]')){throw "No world rendering: reject timing result ($root)"}
    if(-not ($summary -match "res=${Width}x${Height} ")){throw "Actual render size differs from requested ${Width}x${Height}: $root"}
    if($FrameTiming){
        $timing=Get-Content -Raw -LiteralPath "$root/route-timing.json" | ConvertFrom-Json
        if($timing.recording_closed -isnot [bool] -or -not $timing.recording_closed -or
            $timing.csv_io_failed -or $timing.frames -lt 1 -or
            -not(Test-Path -LiteralPath "$root/route-frames.csv")){
            throw "Incomplete full-frame diagnostics: $root"
        }
    }
    if($NormalPresent -and -not ($summary -match 'present_request=auto-(no-)?vsync')){
        throw "Missing actual presentation request: $root"
    }
    if($Capture -and -not(Test-Path -LiteralPath "$root/camera.png")){throw "Missing resident fixed-pose capture: $root"}
    if($Video){
        $frames=@(Get-ChildItem -LiteralPath $root -Filter 'bench-*.png' | Sort-Object Name)
        if($frames.Count -lt 2){throw "Missing motion frames: $root"}
        $concat=[Collections.Generic.List[string]]::new()
        for($i=0;$i -lt $frames.Count;$i++){
            $concat.Add("file '$($frames[$i].Name)'")
            $duration=0.5
            if($i+1 -lt $frames.Count){$duration=([int]($frames[$i+1].BaseName -replace 'bench-','')-[int]($frames[$i].BaseName -replace 'bench-',''))*0.5}
            $concat.Add('duration '+$duration.ToString('0.###',[Globalization.CultureInfo]::InvariantCulture))
        }
        $concat | Set-Content -Encoding ascii "$root/frames.txt"
        & ffmpeg -hide_banner -loglevel error -f concat -safe 0 -i "$root/frames.txt" -fps_mode vfr -c:v libx264 -crf 18 -pix_fmt yuv420p "$root/traversal.mp4"
        if($LASTEXITCODE -ne 0){throw 'Video encoding failed'}
    }
    Write-Output $root
    $summary
} finally {
    if($telemetry -and -not $telemetry.HasExited){$telemetry.Kill();$telemetry.WaitForExit()}
    foreach($name in $previous.Keys){Set-ReviewEnvironment $name $previous[$name]}
}
