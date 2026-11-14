param(
    [string]$Executable = 'target/release/minecraft_style_voxel_prototype.exe',
    [string]$Label = "engine-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [ValidateSet('explore', 'edits')][string]$Scene = 'explore',
    [ValidateRange(10, 1800)][int]$Seconds = 90,
    [ValidateRange(0, 2400)][int]$TimeoutSeconds = 0,
    [uint32]$Seed = 74,
    [switch]$TraceStages
)
# Native diagnostics, with isolated saves and a frozen executable. This measures
# a generated fixture, not save reopening or an authenticated multiplayer join.
$ErrorActionPreference = 'Stop'
if ($TimeoutSeconds -eq 0) { $TimeoutSeconds = $Seconds + 120 }
$repo = Split-Path $PSScriptRoot -Parent
if ($Label -notmatch '^[a-zA-Z0-9_-]+$') { throw 'Label must be a simple directory name' }
$root = Join-Path $repo "target/engine-envelope/$Label"
if (Test-Path -LiteralPath $root) { throw "Evidence already exists: $root" }
$source = if ([IO.Path]::IsPathRooted($Executable)) { $Executable } else { Join-Path $repo $Executable }
$source = (Resolve-Path -LiteralPath $source).Path
New-Item -ItemType Directory -Path $root | Out-Null
$binary = Join-Path $root 'candidate.exe'
Copy-Item -LiteralPath $source -Destination $binary
$gameArgs = @('--benchmark-world=hills', "--benchmark-seed=$Seed",
    '--benchmark-width=1280', '--benchmark-height=720', '--benchmark-warmup=6',
    "--benchmark-duration=$Seconds", '--benchmark-physics', '--benchmark-weather=clear',
    "--benchmark-motion=$(if ($Scene -eq 'explore') { 'orbit' } else { 'static' })")
if ($Scene -eq 'edits') { $gameArgs += '--benchmark-terrain-camera=0,0,8,0,-0.2' }
$info = [Diagnostics.ProcessStartInfo]::new($binary)
$info.WorkingDirectory = $repo
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
# Do not inherit another review's switches or a user's save destination.
foreach ($key in @($info.Environment.Keys)) {
    if ($key -like 'VOXEL_*') { $info.Environment.Remove($key) | Out-Null }
}
$info.Environment['VOXEL_SAVE_DIR'] = Join-Path $root 'save'
$info.Environment['VOXEL_CAPTURE_DIR'] = $root
$info.Environment['VOXEL_RENDER_TIMING'] = '1'
if ($TraceStages) { $info.Environment['VOXEL_FRAME_STAGE_TRACE'] = '1' }
if ($Scene -eq 'edits') { $info.Environment['VOXEL_TERRAIN_EDITS'] = '1' }
foreach ($arg in $gameArgs) { $info.ArgumentList.Add($arg) }
$hardware = @{ cpu = $env:PROCESSOR_IDENTIFIER; gpu = @(); query_error = $null }
try {
    $hardware.cpu = (Get-CimInstance Win32_Processor | ForEach-Object Name) -join '; '
    $hardware.gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion)
} catch { $hardware.query_error = $_.Exception.Message }
$meta = [ordered]@{
    scope = 'Generated native fixture; synthetic edits bypass tool admission. No loading or multiplayer acceptance claim.'
    source_manifest_scope = 'Files observed at launch, not proof of compiler inputs in this concurrently edited checkout.'
    presentation = 'Benchmark disables vsync and automatic quality; default distance/budgets with isolated preferences.'
    started_utc = [DateTime]::UtcNow.ToString('o')
    source_executable = $source
    executable_sha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
    head = (& git -C $repo rev-parse HEAD)
    arguments = $gameArgs
    environment = @{ VOXEL_RENDER_TIMING = '1'; VOXEL_TERRAIN_EDITS = ($Scene -eq 'edits'); VOXEL_FRAME_STAGE_TRACE = [bool]$TraceStages }
    hardware = $hardware
    completed = $false
    fixture_exercised = $false
    timed_out = $false
    exit_code = $null
    peak_sampled_private_bytes = 0L
    peak_sampled_working_set = 0L
    build_contention_observed = $false
}
& git -C $repo status --porcelain=v1 | Set-Content -LiteralPath "$root/tree-status.txt"
# Include untracked native source: HEAD alone cannot identify this shared tree.
$sourceHashes = foreach ($file in Get-ChildItem -LiteralPath "$repo/crates", "$repo/assets" -Recurse -File |
    Where-Object { $_.Extension -in '.rs', '.wgsl', '.toml', '.ron' }) {
    [pscustomobject]@{ path = [IO.Path]::GetRelativePath($repo, $file.FullName); sha256 = (Get-FileHash -LiteralPath $file.FullName).Hash }
}
$sourceHashes | Export-Csv -LiteralPath "$root/source-hashes.csv" -NoTypeInformation
$meta | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$root/run.json"
$streams = [Collections.Generic.List[object]]::new()
$process = $null
$samples = [IO.StreamWriter]::new("$root/process-memory.csv", $false)
$samples.WriteLine('seconds,private_bytes,working_set_bytes,peak_working_set_bytes,cpu_seconds')
$watch = [Diagnostics.Stopwatch]::StartNew()
try {
    $process = [Diagnostics.Process]::Start($info)
    foreach ($pair in @(@('stdout', $process.StandardOutput), @('stderr', $process.StandardError))) {
        $file = [IO.FileStream]::new("$root/$($pair[0]).log", [IO.FileMode]::CreateNew,
            [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite, 4096, $true)
        $streams.Add([pscustomobject]@{ File = $file; Task = $pair[1].BaseStream.CopyToAsync($file) })
    }
    while (-not $process.WaitForExit(500)) {
        $process.Refresh()
        $privateBytes = $process.PrivateMemorySize64
        $workingSet = $process.WorkingSet64
        $meta.peak_sampled_private_bytes = [Math]::Max($meta.peak_sampled_private_bytes, $privateBytes)
        $meta.peak_sampled_working_set = [Math]::Max($meta.peak_sampled_working_set, $workingSet)
        if (Get-Process cargo,rustc,link,rust-lld -ErrorAction SilentlyContinue) {
            $meta.build_contention_observed = $true
        }
        $samples.WriteLine([string]::Format([Globalization.CultureInfo]::InvariantCulture,
            '{0:F3},{1},{2},{3},{4:F3}', $watch.Elapsed.TotalSeconds, $privateBytes,
            $workingSet, $process.PeakWorkingSet64, $process.TotalProcessorTime.TotalSeconds))
        $samples.Flush()
        foreach ($stream in $streams) {
            if ($stream.Task.IsFaulted) { $null = $stream.Task.GetAwaiter().GetResult() }
        }
        if ($watch.Elapsed.TotalSeconds -ge $TimeoutSeconds) {
            $meta.timed_out = $true
            $process.Kill()
            break
        }
    }
    $process.WaitForExit()
    $meta.exit_code = $process.ExitCode
    $meta.completed = -not $meta.timed_out -and $process.ExitCode -eq 0
} finally {
    if ($process -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    foreach ($stream in $streams) {
        try { $null = $stream.Task.GetAwaiter().GetResult() } finally { $stream.File.Dispose() }
    }
    $samples.Dispose()
    $marker = if ($Scene -eq 'edits') { '^\[terrain-edit\]' } else { '^\[benchmark\].*motion=orbit' }
    $meta.fixture_exercised = @(Select-String -LiteralPath "$root/stdout.log", "$root/stderr.log" -Pattern $marker -ErrorAction SilentlyContinue).Count -gt 0
    $meta.completed = $meta.completed -and $meta.fixture_exercised
    $meta['elapsed_seconds'] = $watch.Elapsed.TotalSeconds
    $meta | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$root/run.json"
    if ($process) { $process.Dispose() }
}
Write-Output "Evidence: $root"
if (-not $meta.completed) { throw "Native run failed, timed out, or did not exercise its fixture; see $root/run.json and stderr.log" }
