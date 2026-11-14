param(
    [string]$Executable = 'target/release-fast/minecraft_style_voxel_prototype.exe',
    [string]$Output = ('target/flight-pacing-' + (Get-Date -Format 'yyyyMMdd-HHmmss')),
    [ValidateRange(1, 20)][int]$Repeat = 2,
    [ValidateRange(30, 900)][int]$TimeoutSeconds = 180
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repo
$variables = 'VOXEL_CAPTURE_DIR', 'VOXEL_SAVE_DIR', 'VOXEL_CACHE_DIR', 'VOXEL_ROUTE_TIMING',
    'VOXEL_BENCHMARK_QUIET_INPUT', 'VOXEL_HORIZON_ROUTE', 'VOXEL_RENDER_TIMING',
    'VOXEL_BENCHMARK_NORMAL_PRESENT', 'VOXEL_FRAME_PACER'
$previous = @{}
foreach ($name in $variables) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }

try {
    $source = (Resolve-Path -LiteralPath $Executable).Path
    $root = (New-Item -ItemType Directory -Path $Output).FullName
    $game = Join-Path $root 'voxel.exe'
    Copy-Item -LiteralPath $source -Destination $game
    (Get-FileHash -LiteralPath $game).Hash | Set-Content (Join-Path $root 'executable-sha256.txt')
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $null) }
    $env:VOXEL_CACHE_DIR = Join-Path $root 'cache'
    $env:VOXEL_ROUTE_TIMING = '1'
    $env:VOXEL_BENCHMARK_QUIET_INPUT = '1'

    $summary = foreach ($run in 1..$Repeat) {
        $scenes = if ($run % 2 -eq 1) { 'hills', 'planet' } else { 'planet', 'hills' }
        foreach ($scene in $scenes) {
            $dir = (New-Item -ItemType Directory -Path (Join-Path $root "$run-$scene")).FullName
            $env:VOXEL_CAPTURE_DIR = $dir
            $env:VOXEL_SAVE_DIR = Join-Path $dir 'saves'
            $flight = @(
                "--benchmark-world=$scene", '--benchmark-seed=7', '--benchmark-scale=1m',
                '--benchmark-weather=clear', '--benchmark-review-time=0.5', '--benchmark-sky-time=12',
                '--benchmark-view-chunks=8', '--benchmark-width=960', '--benchmark-height=540',
                '--benchmark-warmup=8', '--benchmark-duration=18', '--benchmark-motion=lod-travel'
            )
            $flight | Set-Content (Join-Path $dir 'arguments.txt')
            $stdout = Join-Path $dir 'stdout.log'
            $process = Start-Process -FilePath $game -ArgumentList $flight -WorkingDirectory $repo `
                -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout `
                -RedirectStandardError (Join-Path $dir 'stderr.log')
            try {
                if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
                    $process.Kill()
                    $process.WaitForExit()
                    throw "Flight timed out for $scene in run $run."
                }
                if ($process.ExitCode -ne 0) { throw "Flight failed for $scene in run $run ($($process.ExitCode))." }
            }
            finally {
                $process.Dispose()
            }
            if (-not (Select-String -LiteralPath $stdout -Pattern '^\[present\] mode=Mailbox$' -Quiet)) {
                throw "Mailbox was not selected for $scene in run $run."
            }
            $timing = Get-Content (Join-Path $dir 'route-timing.json') -Raw | ConvertFrom-Json
            if (!$timing.recording_closed -or $timing.csv_io_failed -or $timing.csv_dropped_frames) {
                throw "Incomplete timing data for $scene in run $run."
            }
            $rows = @(Import-Csv (Join-Path $dir 'route-frames.csv') |
                Where-Object { $_.measuring_at_render -eq 'true' -and $_.render_ok -eq 'true' })
            $log = Get-Content -LiteralPath $stdout -Raw
            $match = [regex]::Match($log, 'p95_frame_ms=([\d.]+) p99_frame_ms=([\d.]+) max_frame_ms=([\d.]+)')
            if ($rows.Count -eq 0 -or !$match.Success) { throw "No completed measurement for $scene in run $run." }
            [pscustomobject]@{
                run = $run
                scene = $scene
                p95_ms = $match.Groups[1].Value
                p99_ms = $match.Groups[2].Value
                worst_ms = $match.Groups[3].Value
                retry_redraw_ms = [math]::Round(($rows | Measure-Object gate_retry_redraw_ms -Sum).Sum, 3)
                gate_poll_ms = [math]::Round(($rows | Measure-Object gate_wait_ms -Sum).Sum, 3)
                retry_late_ms = [math]::Round(($rows | Measure-Object gpu_retry_late_ms -Sum).Sum, 3)
                dispatch_max_ms = ($rows | Measure-Object redraw_dispatch_ms -Maximum).Maximum
                acquire_max_ms = ($rows | Measure-Object acquire_ms -Maximum).Maximum
            }
        }
    }
    $summary | Export-Csv -NoTypeInformation (Join-Path $root 'summary.csv')
    $summary | Format-Table
}
finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
    Pop-Location
}
