param([switch]$Measure)
$ErrorActionPreference = 'Stop'
$target = Join-Path $PSScriptRoot 'target'
$evidence = Join-Path $target 'verification'
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$reference = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'reference-inputs.json') -Raw | ConvertFrom-Json -AsHashtable
$report = @{
    started_utc = [DateTime]::UtcNow.ToString('o')
    succeeded = $false
    checks = @()
    source_sha256 = @{}
    scope = 'Offline routine projection and agenda fixtures only; no game authority, saves, multiplayer or performance acceptance.'
    integration_notes = @(
        'Wheel set-semantics fix is integrated into the dormant kernel; game remains on BTreeSet.',
        'Original kernel wake logic is stale; isolated facts model matches the compiled current function.'
    )
}
Push-Location $PSScriptRoot
try {
    $report.toolchain = (& rustc --version)
    foreach ($path in $reference.Keys) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $repo $path) -Algorithm SHA256).Hash
        $report.source_sha256[$path] = $hash
        if ($hash -ne $reference[$path]) { throw "Unreviewed source change: $path. Review the projection before refreshing reference-inputs.json." }
    }
    $localInputs = @(Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'src'),(Join-Path $PSScriptRoot 'tests'),(Join-Path $PSScriptRoot 'examples') -Filter '*.rs' -Recurse -File | ForEach-Object { $_.FullName })
    $localInputs += @('build.rs','Cargo.toml','Cargo.lock','Verify.ps1','reference-inputs.json') | ForEach-Object { Join-Path $PSScriptRoot $_ }
    foreach ($path in $localInputs) {
        $relative = [IO.Path]::GetRelativePath($repo, $path).Replace('\','/')
        $report.source_sha256[$relative] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    }
    $report.other_processes_at_start = @(Get-Process cargo,rustc,minecraft_style_voxel_prototype -ErrorAction SilentlyContinue | Select-Object Id,ProcessName)
    $names = @('test', 'clippy', 'kernel_test', 'kernel_clippy', 'kernel_check')
    if ($Measure) { $names += 'queue_bench' }
    foreach ($name in $names) {
        $cargoArgs = @($name, '--offline', '--locked', '--target-dir', $target)
        if ($name -eq 'clippy') { $cargoArgs += @('--all-targets', '--', '-D', 'warnings') }
        if ($name.StartsWith('kernel_')) {
            $verb = switch ($name) { 'kernel_test' { 'test' }; 'kernel_clippy' { 'clippy' }; 'kernel_check' { 'check' } }
            $cargoArgs = @($verb, '--offline', '--locked', '--manifest-path', (Join-Path $repo 'crates/civ/Cargo.toml'), '-p', 'engine_civ', '--target-dir', (Join-Path $target 'kernel'))
            if ($name -eq 'kernel_test') { $cargoArgs += '--lib' } else { $cargoArgs += '--all-targets' }
            if ($name -eq 'kernel_clippy') { $cargoArgs += @('--', '-D', 'warnings') }
        }
        if ($name -eq 'queue_bench') { $cargoArgs = @('run', '--offline', '--locked', '--release', '--target-dir', $target, '--example', 'queue_bench') }
        $watch = [Diagnostics.Stopwatch]::StartNew()
        & cargo @cargoArgs *> (Join-Path $evidence "$name.log")
        $exit = $LASTEXITCODE
        $watch.Stop()
        $report.checks += @{ name = $name; exit_code = $exit; seconds = $watch.Elapsed.TotalSeconds }
        Get-Content -LiteralPath (Join-Path $evidence "$name.log") | Select-Object -Last 8
        if ($exit -ne 0) { throw "$name failed with exit code $exit" }
    }
    foreach ($path in $report.source_sha256.Keys) {
        $current = (Get-FileHash -LiteralPath (Join-Path $repo $path) -Algorithm SHA256).Hash
        if ($current -ne $report.source_sha256[$path]) { throw "Input changed during verification: $path" }
    }
    if ($Measure) {
        $report.queue_bench = @(Get-Content -LiteralPath (Join-Path $evidence 'queue_bench.log') | Where-Object { $_.StartsWith('{') } | ConvertFrom-Json)
    }
    $report.succeeded = $true
    Write-Output "Verification evidence: $evidence"
} catch {
    $report.failure = $_.Exception.Message
    throw
} finally {
    $report.finished_utc = [DateTime]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidence 'result.json') -Encoding utf8
    Pop-Location
}
