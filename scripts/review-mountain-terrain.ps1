param(
    [string]$Executable,
    [string]$OutputDirectory,
    [ValidateSet('hills','ridged')][string]$World = 'hills',
    [uint32]$Generator = 253,
    [uint32]$Seed = 2738958700,
    [ValidateSet('1','0.5','0.25')][string]$Scale = '1',
    [string]$Camera = '464,-720,-710,0,-0.27'
)
# One fresh generated-world capture. The manifest and engine logs make a
# baseline/candidate comparison reproducible without opening a player's save.
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot
if (-not $Executable) { $Executable = Join-Path $repo 'target/release-fast/minecraft_style_voxel_prototype.exe' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repo ('target/mountain-review/' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff'))
}
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new output directory to preserve existing evidence.' }
$null = New-Item -ItemType Directory -Path $OutputDirectory
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$saveDirectory = Join-Path $OutputDirectory 'scratch-saves'
$null = New-Item -ItemType Directory -Path $saveDirectory
$options = @{
    VOXEL_SAVE_DIR = $saveDirectory
    VOXEL_CAPTURE_DIR = $OutputDirectory
    WGPU_BACKEND = 'vulkan'
    VOXEL_TERRAIN_ROUTE = '0'
    VOXEL_TERRAIN_EDITS = '0'
    VOXEL_CAVE_FLOOD = '0'
}
$previous = @{}
try {
    foreach ($key in $options.Keys) {
        $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
        [Environment]::SetEnvironmentVariable($key, $options[$key], 'Process')
    }
    $arguments = @("--benchmark-world=$World", "--benchmark-generator-version=$Generator",
        "--benchmark-seed=$Seed", "--benchmark-scale=$Scale", "--benchmark-terrain-camera=$Camera",
        '--benchmark-width=1280','--benchmark-height=720','--benchmark-view-chunks=12',
        '--benchmark-weather=clear','--benchmark-review-time=0.5','--benchmark-sky-time=12',
        '--benchmark-cloud-divisor=3','--benchmark-warmup=20','--benchmark-duration=20',
        '--benchmark-capture-frames','--benchmark-capture-interval=10')
    [ordered]@{ executable=$Executable; sha256=(Get-FileHash -LiteralPath $Executable).Hash;
        arguments=$arguments; environment=$options; started_utc=[DateTime]::UtcNow.ToString('o') } |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'manifest.json')
    $process = Start-Process -FilePath $Executable -ArgumentList $arguments -WorkingDirectory $repo `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'stdout.log') `
        -RedirectStandardError (Join-Path $OutputDirectory 'stderr.log')
    [ordered]@{ pid=$process.Id; evidence=$OutputDirectory } | ConvertTo-Json
} finally {
    foreach ($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }
}
