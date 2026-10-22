param(
    [ValidateRange(0,2)] [int]$Variant = 0,
    [ValidateRange(0,2)] [int]$Tier = 0,
    [ValidateSet('Showcase','Combat','Orbit','Duel','Manual')] [string]$Mode = 'Combat',
    [switch]$Enraged,
    [string]$Label = 'final',
    [int]$Seconds = 24,
    [ValidateRange(1,20)] [int]$CaptureHz = 5,
    [ValidateRange(640,3840)] [int]$Width = 1600,
    [ValidateRange(360,2160)] [int]$Height = 900,
    [ValidateRange(0,60)] [int]$Warmup = 6,
    [ValidateRange(1,16)] [int]$ViewChunks = 4,
    [string]$Executable = 'target/prism-hart/final.exe'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe = (Resolve-Path -LiteralPath $Executable).Path
$root = Join-Path $repo "target/prism-hart/$Label-$Mode-v$Variant-t$Tier"
if ($Enraged) { $root += '-enraged' }
if (Test-Path -LiteralPath $root) { throw "Evidence exists: $root" }
New-Item -ItemType Directory -Path $root -Force | Out-Null
$captureInterval = if ($Mode -eq 'Showcase') { '1' } else { (1.0 / $CaptureHz).ToString('0.######', [Globalization.CultureInfo]::InvariantCulture) }
$argsList = @("--benchmark-hart-review=$Variant,$Tier", "--benchmark-width=$Width", "--benchmark-height=$Height",
    "--benchmark-view-chunks=$ViewChunks",
    "--benchmark-warmup=$Warmup", "--benchmark-duration=$Seconds", '--benchmark-capture-hud',
    '--benchmark-weather=clear', '--benchmark-sky-time=12',
    $(if ($Mode -eq 'Manual') { '--benchmark-physics' } else { "--benchmark-capture-interval=$captureInterval" }))
$names = @('VOXEL_CAPTURE_DIR','VOXEL_HART_SHOWCASE','VOXEL_HART_ORBIT','VOXEL_HART_ENRAGED','VOXEL_HART_CAST')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:VOXEL_CAPTURE_DIR = $root
    $env:VOXEL_HART_SHOWCASE = if ($Mode -eq 'Showcase') { '1' } else { $null }
    $env:VOXEL_HART_ORBIT = if ($Mode -in @('Orbit','Duel')) { '1' } else { $null }
    $env:VOXEL_HART_CAST = if ($Mode -eq 'Duel') { '1' } else { $null }
    $env:VOXEL_HART_ENRAGED = if ($Enraged) { '1' } else { $null }
    [ordered]@{
        executable=$exe; sha256=(Get-FileHash -LiteralPath $exe).Hash
        mode=$Mode; variant=$Variant; tier=$Tier; starts_enraged=[bool]$Enraged; capture_interval=$captureInterval
        args=$argsList; started_utc=[DateTime]::UtcNow.ToString('o')
        note='Showcase holds an idle pose. Combat/Orbit run actual attacks, collision, damage and respawn. Duel adds automatic aiming/casting through normal wand mana and cooldowns. Enraged is an explicitly low-health starting fixture.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    $windowStyle=if ($Mode -eq 'Manual') { 'Normal' } else { 'Hidden' }
    $process = Start-Process -FilePath $exe -ArgumentList $argsList -WorkingDirectory $repo -WindowStyle $windowStyle -PassThru `
        -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    $null = $process.Handle
    $peak = 0L
    $wall=[Diagnostics.Stopwatch]::StartNew()
    while (-not $process.WaitForExit(500)) {
        $process.Refresh(); $peak=[Math]::Max($peak,$process.PrivateMemorySize64)
        if ($wall.Elapsed.TotalSeconds -gt [Math]::Max(90,$Seconds+$Warmup+45)) {
            Stop-Process -Id $process.Id
            throw "Native review exceeded its wall-clock budget; incomplete evidence retained at $root"
        }
    }
    [ordered]@{ exit_code=$process.ExitCode; peak_private_bytes=$peak } | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $root 'process.json')
    if ($null -ne $process.ExitCode -and $process.ExitCode -ne 0) { throw "Game exited $($process.ExitCode)" }
    if ($Mode -ne 'Manual' -and -not (Select-String -LiteralPath (Join-Path $root 'stdout.log') -SimpleMatch '[benchmark] avg_fps=' -Quiet)) {
        throw "Review did not finish: $root"
    }
    Write-Output "Evidence: $root"
    Select-String -LiteralPath (Join-Path $root 'stdout.log') -Pattern '\[hart-review\]|avg_fps=|gpu_avg_ms' | ForEach-Object { $_.Line }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$previous[$name]) }
}
