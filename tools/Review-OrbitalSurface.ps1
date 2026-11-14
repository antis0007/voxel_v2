param(
    [string]$Label = 'surface-baseline',
    [string]$Binary = 'target/landscape-cycles-review/final/axiom-286-smooth.exe',
    [ValidateSet('legacy','resolution','procedural','cached')][string]$Surface = 'legacy',
    [double]$Altitude = 26000,
    [double]$X = 323325, [double]$Z = 2127,
    [double]$Time = 0.635417,
    [int]$Warmup = 25, [int]$Duration = 35,
    [switch]$NoCaptures
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
$review = Join-Path $workspace "target/orbital-surface-review/$Label"
$binaryPath = if ([IO.Path]::IsPathRooted($Binary)) { $Binary } else { Join-Path $workspace $Binary }
foreach ($part in @('','captures','save','cache')) {
    New-Item -ItemType Directory -Force -Path (Join-Path $review $part) | Out-Null
}
$env:VOXEL_SAVE_DIR = Join-Path $review 'save'
$env:VOXEL_CACHE_DIR = Join-Path $review 'cache'
$env:VOXEL_CAPTURE_DIR = Join-Path $review 'captures'
$env:VOXEL_GLOBE_SURFACE = $Surface
$env:VOXEL_BENCHMARK_NORMAL_PRESENT = '1'
$env:VOXEL_GAMEPLAY_TRACE = '1'
$env:VOXEL_REVIEW_NO_ORBITAL = '0'
$env:VOXEL_CAPTURE_GENERATION = '0'
$env:VOXEL_PLANET_FLIGHT_REVIEW = '0'
$env:VOXEL_PLANET_WALK_REVIEW = ''
$arguments = @('--benchmark-world=planet','--benchmark-scale=1','--benchmark-seed=74',
    '--benchmark-generator-version=286','--benchmark-view-chunks=12',
    '--benchmark-weather=clear','--benchmark-review-day=135',"--benchmark-review-time=$Time",
    "--benchmark-orbital-time=$(135 + $Time)",'--benchmark-width=1600','--benchmark-height=900',
    "--benchmark-warmup=$Warmup","--benchmark-duration=$Duration",
    "--benchmark-terrain-camera=$X,$Z,$Altitude,0,-1.50")
if (!$NoCaptures) { $arguments += '--benchmark-capture-frames','--benchmark-capture-interval=10' }
$process = Start-Process -FilePath $binaryPath -WorkingDirectory $workspace -ArgumentList $arguments -WindowStyle Hidden -RedirectStandardOutput (Join-Path $review 'stdout.log') -RedirectStandardError (Join-Path $review 'stderr.log') -PassThru
[pscustomobject]@{Process=$process.Id;Surface=$Surface;Started=(Get-Date -Format o);Binary=$binaryPath;SHA256=(Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash;Arguments=$arguments} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $review 'receipt.json')
Get-Content -LiteralPath (Join-Path $review 'receipt.json')
