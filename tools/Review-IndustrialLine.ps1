param(
    [string]$Executable = 'target/debug/minecraft_style_voxel_prototype.exe',
    [string[]]$Scales = @('1', '0.5', '0.25'),
    [int]$TimeoutSeconds = 720,
    [ValidateSet('Industry', 'Processors')][string]$Mode = 'Industry'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$source = if ([IO.Path]::IsPathRooted($Executable)) { $Executable } else { Join-Path $repo $Executable }
$exe = (Resolve-Path -LiteralPath $source).Path
$root = Join-Path $repo ('target/industrial-review/' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $root | Out-Null
$candidate = Join-Path $root 'candidate.exe'
Copy-Item -LiteralPath $exe -Destination $candidate
(Get-FileHash -LiteralPath $candidate).Hash | Set-Content -LiteralPath (Join-Path $root 'candidate.sha256')
$environment = @('VOXEL_SAVE_DIR', 'VOXEL_CAPTURE_DIR', 'VOXEL_INDUSTRIAL_REVIEW', 'VOXEL_REVIEW_SKIP_CLOUDS', 'VOXEL_REVIEW_SKIP_HORIZON')
$prior = @{}
foreach ($name in $environment) { $prior[$name] = [Environment]::GetEnvironmentVariable($name) }
$failed = @()
try {
    $env:VOXEL_INDUSTRIAL_REVIEW = if ($Mode -eq 'Processors') { 'processors' } else { '1' }
    $marker = if ($Mode -eq 'Processors') { '[processors-review]' } else { '[industrial-review]' }
    $env:VOXEL_REVIEW_SKIP_CLOUDS = '1'
    $env:VOXEL_REVIEW_SKIP_HORIZON = '1'
    foreach ($scale in $Scales) {
        $run = Join-Path $root $scale
        New-Item -ItemType Directory -Path $run | Out-Null
        $env:VOXEL_SAVE_DIR = Join-Path $run 'saves'
        $env:VOXEL_CAPTURE_DIR = $run
        $stdout = Join-Path $run 'stdout.log'
        $stderr = Join-Path $run 'stderr.log'
        $arguments = @('--benchmark-world=superflat', "--benchmark-scale=$scale", '--benchmark-motion=static',
            '--benchmark-width=960', '--benchmark-height=540', '--benchmark-view-chunks=3', '--benchmark-warmup=1',
            "--benchmark-duration=$TimeoutSeconds", '--benchmark-review-time=0.5', '--benchmark-weather=clear', '--benchmark-physics')
        Write-Output "Running ${scale} m industrial review at $run"
        $game = Start-Process -FilePath $candidate -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        $null = $game.Handle
        if (!$game.WaitForExit($TimeoutSeconds * 1000)) { $game.Kill(); $game.WaitForExit(); $failed += $scale }
        $reviewLog = if ($Mode -eq 'Processors') { $stderr } else { $stdout }
        Select-String -LiteralPath $reviewLog -SimpleMatch $marker | ForEach-Object { $_.Line }
        if ($game.ExitCode -ne 0 -or !(Select-String -LiteralPath $reviewLog -SimpleMatch "$marker PASS" -Quiet)) {
            if ($scale -notin $failed) { $failed += $scale }
            Get-Content -LiteralPath $stderr -Tail 8
        }
    }
} finally {
    foreach ($name in $environment) { [Environment]::SetEnvironmentVariable($name, $prior[$name]) }
}
if ($failed.Count) { throw "Industrial review failed at $($failed -join ', ') m. Logs at $root" }
