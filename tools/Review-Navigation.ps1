param(
    [string]$Exe = "target/release-fast/minecraft_style_voxel_prototype.exe",
    [string]$Output = "target/navigation-review/native",
    [int]$Seconds = 80,
    [switch]$Manual
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$binary = (Resolve-Path -LiteralPath (Join-Path $root $Exe)).Path
$destination = [IO.Path]::GetFullPath((Join-Path $root $Output))
New-Item -ItemType Directory -Force $destination | Out-Null
$reviewBinary = Join-Path $destination 'navigation-review.exe'
Copy-Item -LiteralPath $binary -Destination $reviewBinary
$names = @('VOXEL_REVIEW_NAVIGATION', 'VOXEL_SAVE_DIR', 'VOXEL_CAPTURE_DIR')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    $env:VOXEL_REVIEW_NAVIGATION = if ($Manual) { 'manual' } else { '1' }
    $env:VOXEL_SAVE_DIR = Join-Path $destination 'disposable-saves'
    $env:VOXEL_CAPTURE_DIR = $destination
    $arguments = @('--benchmark-world=planet', '--benchmark-seed=1337',
        '--benchmark-generator-version=272', '--benchmark-terrain-camera=1300,-500,250,0,-0.1',
        '--benchmark-review-time=0.8', '--benchmark-weather=clear',
        '--benchmark-width=1600', '--benchmark-height=900', '--benchmark-view-chunks=8',
        '--benchmark-warmup=5', "--benchmark-duration=$Seconds", '--benchmark-capture-hud',
        $(if ($Manual) { '--benchmark-capture-interval=30' } else { '--benchmark-capture-interval=3' }))
    $process = Start-Process -FilePath $reviewBinary -ArgumentList $arguments -WorkingDirectory $root `
        -RedirectStandardOutput (Join-Path $destination 'stdout.log') `
        -RedirectStandardError (Join-Path $destination 'stderr.log') -PassThru
    [pscustomobject]@{ ProcessId = $process.Id; Output = $destination; Executable = $reviewBinary }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
}
