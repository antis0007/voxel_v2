param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [uint32]$Generator = 271,
    [uint32]$Seed = 74,
    [ValidateSet('1','0.5','0.25')][string]$Scale = '1',
    [string]$Camera = '344059.90,19852.79,-1330,0.685844,-0.48',
    [ValidateSet('none','clay','albedo')][string]$Diagnostic = 'none'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if(Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new evidence directory.' }
$null = New-Item -ItemType Directory -Path $OutputDirectory
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$options = @{
    VOXEL_SAVE_DIR = (Join-Path $OutputDirectory 'scratch-saves')
    VOXEL_CACHE_DIR = (Join-Path $OutputDirectory 'cache')
    VOXEL_CAPTURE_DIR = $OutputDirectory
    WGPU_BACKEND = 'vulkan'
    VOXEL_TERRAIN_ROUTE = '0'
    VOXEL_TERRAIN_EDITS = '0'
    VOXEL_CAVE_FLOOD = '0'
}
$previous = @{}
try {
    foreach($key in $options.Keys) {
        $previous[$key] = [Environment]::GetEnvironmentVariable($key,'Process')
        [Environment]::SetEnvironmentVariable($key,$options[$key],'Process')
    }
    $arguments = @('--benchmark','--benchmark-world=planet',"--benchmark-generator-version=$Generator",
        "--benchmark-seed=$Seed","--benchmark-scale=$Scale","--benchmark-terrain-camera=$Camera",
        '--benchmark-width=1280','--benchmark-height=720','--benchmark-view-chunks=24',
        '--benchmark-weather=clear','--benchmark-review-day=135','--benchmark-review-time=0.601389',
        '--benchmark-orbital-time=135.601389','--benchmark-cloud-divisor=3',
        '--benchmark-warmup=24','--benchmark-duration=12','--benchmark-capture-frames',
        '--benchmark-capture-interval=6')
    if($Diagnostic -ne 'none') { $arguments += "--benchmark-debug=$Diagnostic" }
    [ordered]@{executable=$Executable;sha256=(Get-FileHash -LiteralPath $Executable).Hash;
        arguments=$arguments;environment=$options;started_utc=[DateTime]::UtcNow.ToString('o')} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'manifest.json')
    $process = Start-Process -FilePath $Executable -ArgumentList $arguments -WorkingDirectory $repo `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'stdout.log') `
        -RedirectStandardError (Join-Path $OutputDirectory 'stderr.log')
    [ordered]@{pid=$process.Id;evidence=$OutputDirectory} | ConvertTo-Json
} finally {
    foreach($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key,$previous[$key],'Process') }
}
