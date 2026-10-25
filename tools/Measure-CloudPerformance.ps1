param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateSet('clear','partly','overcast','storm','winter')][string]$Weather = 'partly',
    [int]$Width = 1280,
    [int]$Height = 720,
    [int]$Seconds = 12,
    [int]$Warmup = 10,
    [int]$CloudDivisor = 3,
    [switch]$Capture,
    [switch]$AdaptiveClouds,
    [string]$WorkingDirectory
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
if ($WorkingDirectory) { $repo = (Resolve-Path -LiteralPath $WorkingDirectory).Path }
$binary = (Resolve-Path -LiteralPath $Exe).Path
$root = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $root) { throw "Use a new output directory: $root" }
function Get-OtherRenderer([int]$OwnedPid = 0) {
    Get-Process -ErrorAction SilentlyContinue | Where-Object {
        $_.Id -ne $OwnedPid -and ($_.ProcessName -match 'minecraft_style_voxel|clouds|^base-[a-f0-9]+$|^(before|after)(-|$)|^(bounds|shared-taps|ordered|medium|layers|masked|scalar|combined|harmonics|bounded|coherent|registers|specialized|(morph|sampling)-only)$') -and -not $_.HasExited
    }
}
# Peer sessions have their own isolated A/B binaries. Yield the GPU to them;
# never terminate their work or count overlapping runs as a speedup.
$idleDeadline = [DateTime]::UtcNow.AddMinutes(8)
$idleSince = [DateTime]::UtcNow
$lastPeerIds = ''
$quietSeconds = 8
while (([DateTime]::UtcNow - $idleSince).TotalSeconds -lt $quietSeconds) {
    $peers = @(Get-OtherRenderer)
    $peerIds = ($peers | ForEach-Object { "$($_.ProcessName):$($_.Id)" }) -join ', '
    if ($peerIds -ne $lastPeerIds) { Write-Output "[isolation] peers=[$peerIds]"; $lastPeerIds = $peerIds }
    if ($peers.Count -gt 0) { $idleSince = [DateTime]::UtcNow; $quietSeconds = 30 }
    if ([DateTime]::UtcNow -ge $idleDeadline) { throw 'GPU review is still owned by a peer session; retry later' }
    Start-Sleep -Milliseconds 500
}
New-Item -ItemType Directory -Path $root | Out-Null
$arguments = @('--benchmark', '--benchmark-world=hills', '--benchmark-seed=2738958700',
    '--benchmark-view-chunks=16', '--benchmark-motion=static', '--benchmark-render-scale=100',
    '--benchmark-terrain-camera=0,0,90,0,0.22', '--benchmark-review-time=0.5',
    '--benchmark-sky-time=12', "--benchmark-cloud-divisor=$CloudDivisor",
    "--benchmark-weather=$Weather", "--benchmark-width=$Width", "--benchmark-height=$Height",
    "--benchmark-warmup=$Warmup", "--benchmark-duration=$Seconds")
if ($Capture) { $arguments += '--benchmark-capture-frames' }
if ($AdaptiveClouds) { $arguments += '--benchmark-adaptive-clouds' }
# Keep player saves/preferences and unrelated review overrides out of the run.
$previous = @{}
Get-ChildItem Env:VOXEL_* | ForEach-Object { $previous[$_.Name] = $_.Value; Remove-Item -LiteralPath "Env:$($_.Name)" }
$process = $null
try {
    $env:VOXEL_CAPTURE_DIR = $root
    $env:VOXEL_SAVE_DIR = Join-Path $root 'generated-world'
    $env:VOXEL_TERRAIN_WIND = '0'
    [ordered]@{
        utc = [DateTime]::UtcNow.ToString('o'); executable = $binary
        working_directory = $repo
        sha256 = (Get-FileHash -LiteralPath $binary).Hash; arguments = $arguments
        environment = @{ VOXEL_CAPTURE_DIR=$root; VOXEL_SAVE_DIR=$env:VOXEL_SAVE_DIR; VOXEL_TERRAIN_WIND='0' }
        capture_run = [bool]$Capture
        adaptive_clouds = [bool]$AdaptiveClouds
        note = 'Fixed scene quality and uncapped presentation; adaptive cloud resolution is opt-in. Capture runs are separate from timing runs.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    $process = Start-Process -FilePath $binary -ArgumentList $arguments -WorkingDirectory $repo `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'stdout.log') `
        -RedirectStandardError (Join-Path $root 'stderr.log')
    $null = $process.Handle
    $deadline = [DateTime]::UtcNow.AddSeconds($Warmup + $Seconds + 180)
    $overlap = @{}
    while (-not $process.WaitForExit(500)) {
        foreach ($peer in @(Get-OtherRenderer $process.Id)) { $overlap[[string]$peer.Id] = $peer.ProcessName }
        if ($overlap.Count -gt 0) {
            $overlap | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'overlap.json')
            Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
            throw 'A peer GPU review started; stopped this owned run and yielded the GPU'
        }
        if ([DateTime]::UtcNow -ge $deadline) {
            Stop-Process -Id $process.Id
            throw "Benchmark exceeded its deadline; stopped owned process $($process.Id)"
        }
    }
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        Get-Content -LiteralPath (Join-Path $root 'stderr.log') -Tail 30
        throw "Benchmark exited with $($process.ExitCode)"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $root 'frames.csv'))) { throw 'Benchmark did not produce frame samples' }
    Select-String -LiteralPath (Join-Path $root 'stdout.log') -Pattern '^\[benchmark\]' | ForEach-Object { $_.Line }
} finally {
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -ErrorAction SilentlyContinue }
    Get-ChildItem Env:VOXEL_* | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" }
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
}
