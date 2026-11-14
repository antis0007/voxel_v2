param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Label,
    [ValidateSet('1','0.5','0.25')][string]$Scale = '1',
    [string]$Camera = '0,0,2,0,-0.14',
    [string]$DebugView = 'none',
    [double]$TimeOfDay = 0.5,
    [int]$Warmup = 16,
    [int]$Seconds = 12,
    [switch]$Mountain,
    [switch]$Motion,
    [switch]$NoCapture,
    [string]$Repo = (Split-Path $PSScriptRoot -Parent),
    [string]$EvidenceRoot = ''
)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (-not $EvidenceRoot) { $EvidenceRoot = Join-Path $Repo 'target/material-distance-review' }
$Repo = (Resolve-Path -LiteralPath $Repo).Path
if ($Label -notmatch '^[a-zA-Z0-9_.-]+$') { throw 'Use a simple unique label.' }
$out = Join-Path $EvidenceRoot $Label
if (Test-Path -LiteralPath $out) { throw 'Evidence exists; use a new label.' }
$null = New-Item -ItemType Directory -Path $out -Force
$save = Join-Path $out 'scratch-saves'
$null = New-Item -ItemType Directory -Path $save
$envMap = @{
    WGPU_BACKEND='vulkan'; VOXEL_SAVE_DIR=$save; VOXEL_CAPTURE_DIR=$out;
    VOXEL_BENCHMARK_QUIET_INPUT='1'; VOXEL_TERRAIN_EDITS='0';
    VOXEL_MATERIAL_DISTANCE_REVIEW=$(if($Mountain){'0'}else{'1'});
    VOXEL_TERRAIN_ROUTE=$(if($Motion){'1'}else{'0'}); VOXEL_CAVE_FLOOD='0'
}
$world = if($Mountain){'hills'}else{'superflat'}
$arguments = @("--benchmark-world=$world",
    '--benchmark-seed=2738958700',
    "--benchmark-scale=$Scale","--benchmark-terrain-camera=$Camera",
    '--benchmark-width=1280','--benchmark-height=720','--benchmark-view-chunks=12',
    '--benchmark-weather=clear',"--benchmark-review-time=$TimeOfDay",
    '--benchmark-sky-time=12','--benchmark-cloud-divisor=3',
    "--benchmark-warmup=$Warmup","--benchmark-duration=$Seconds")
if($DebugView -ne 'none') { $arguments += "--benchmark-debug=$DebugView" }
if (-not $NoCapture) { $arguments += @('--benchmark-capture-frames','--benchmark-capture-interval=6') }
$old = @{}
try {
    foreach($key in $envMap.Keys) {
        $old[$key] = [Environment]::GetEnvironmentVariable($key,'Process')
        [Environment]::SetEnvironmentVariable($key,$envMap[$key],'Process')
    }
    $busy = @(Get-Process cargo,rustc,minecraft_style_voxel_prototype -ErrorAction SilentlyContinue | Select-Object Id,ProcessName)
    [ordered]@{ executable=$exe; sha256=(Get-FileHash $exe).Hash; arguments=$arguments;
        environment=$envMap; other_processes=$busy; started_utc=[DateTime]::UtcNow.ToString('o');
        timing_note='GPU fields are selected timestamped passes, not total GPU frame time.' } |
        ConvertTo-Json -Depth 5 | Set-Content (Join-Path $out 'manifest.json')
    $process = Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $Repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $out 'stdout.log') -RedirectStandardError (Join-Path $out 'stderr.log')
    [ordered]@{ pid=$process.Id; evidence=$out } | ConvertTo-Json
} finally {
    foreach($key in $old.Keys) { [Environment]::SetEnvironmentVariable($key,$old[$key],'Process') }
}
