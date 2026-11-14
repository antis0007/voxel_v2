param(
    [Parameter(Mandatory)][string]$Capture,
    [ValidateRange(1, 1000)][double]$StallMilliseconds = 50
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $Capture).Path
$culture = [Globalization.CultureInfo]::InvariantCulture
$run = Get-Content -LiteralPath (Join-Path $root 'run.json') -Raw | ConvertFrom-Json
$recording = Get-Content -LiteralPath (Join-Path $root 'route-timing.json') -Raw | ConvertFrom-Json
$frames = @(Import-Csv -LiteralPath (Join-Path $root 'route-frames.csv') |
    Where-Object { $_.measuring_at_render -eq 'true' -and $_.render_ok -eq 'true' })
$cadencePath = Join-Path $root 'frames.csv'
$cadenceSource = 'frames.csv (complete benchmark sample export)'
if (Test-Path -LiteralPath $cadencePath) {
    $cadenceFrames = @(Import-Csv -LiteralPath $cadencePath)
} else {
    $cadenceFrames = $frames
    $cadenceSource = 'retained warmed route CSV rows only; check csv_complete'
}
$times = @($cadenceFrames | ForEach-Object { [double]::Parse($_.total_ms, $culture) } | Sort-Object)
function Quantile([double]$fraction) {
    if ($times.Count -eq 0) { return $null }
    return $times[[int][Math]::Round(($times.Count - 1) * $fraction)]
}
$gaps = [Collections.Generic.List[object]]::new()
$previous = $null
$eventName = $null
$traceRows = 0
$gapCount = 0
foreach ($line in [IO.File]::ReadLines((Join-Path $root 'stderr.log'))) {
    if ($line -notmatch '^\[frame-stage\] t=([0-9.]+) (\S+)$') { continue }
    $at = [double]::Parse($Matches[1], $culture)
    $stage = $Matches[2]
    $traceRows++
    if ($null -ne $previous) {
        $ms = ($at - $previous.at) * 1000
        if ($ms -ge $StallMilliseconds) {
            $gapCount++
            $gaps.Add([pscustomobject]@{
                from_seconds = $previous.at; to_seconds = $at
                milliseconds = [Math]::Round($ms, 3)
                from_stage = $previous.stage; to_stage = $stage
                event = $eventName
                scope = $(if ($null -ne $eventName) { 'event callback' }
                    elseif ($previous.stage -match '^event-.*-done$') { 'between event callbacks' }
                    else { 'unknown (older trace or incomplete scope)' })
            })
            # Keep the worst intervals without retaining an unbounded trace.
            if ($gaps.Count -gt 32) {
                $smallest = 0
                for ($i = 1; $i -lt $gaps.Count; $i++) {
                    if ($gaps[$i].milliseconds -lt $gaps[$smallest].milliseconds) { $smallest = $i }
                }
                $gaps.RemoveAt($smallest)
            }
        }
    }
    if ($stage -match '^event-(redraw|resize|wait|window|other)$') { $eventName = $stage }
    elseif ($stage -match '^event-.*-done$') { $eventName = $null }
    $previous = @{ at = $at; stage = $stage }
}
$worst = @($frames | Sort-Object { [double]::Parse($_.total_ms, $culture) } -Descending |
    Select-Object -First 10 frame,seconds,total_ms,redraw_ms,outside_redraw_ms,stream_ms,horizon_ms,render_cpu_ms)
$summary = [ordered]@{
    scope = 'Cadence uses the benchmark sample export when available; route spans may omit dropped rows. Trace gaps include startup and synchronous trace IO. No root-cause or performance-improvement claim.'
    executable_sha256 = $run.executable_sha256
    run_completed = $run.completed
    fixture_exercised = $run.fixture_exercised
    build_contention_observed = $run.build_contention_observed
    csv_complete = ($recording.recording_closed -and -not $recording.csv_io_failed -and -not $recording.csv_dropped_frames)
    csv_dropped_frames = $recording.csv_dropped_frames
    retained_warmed_route_rows = $frames.Count
    cadence_source = $cadenceSource
    measured_frames = $times.Count
    p50_ms = Quantile 0.5; p95_ms = Quantile 0.95; p99_ms = Quantile 0.99
    max_ms = $(if ($times.Count) { $times[-1] } else { $null })
    stall_threshold_ms = $StallMilliseconds
    stalled_frames = @($times | Where-Object { $_ -ge $StallMilliseconds }).Count
    worst_route_frames = $worst
    trace_rows = $traceRows
    trace_stalls = $gapCount
    worst_trace_intervals = @($gaps | Sort-Object milliseconds -Descending)
}
$summary | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'diagnostic-summary.json')
$summary | ConvertTo-Json -Depth 5
