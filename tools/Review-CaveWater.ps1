param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(3,60)][int]$Seconds = 8,
    [ValidateRange(1,5)][int]$Rounds = 1
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$exe = (Resolve-Path -LiteralPath $Executable).Path
$root = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $root) { throw "Evidence already exists: $root" }
New-Item -ItemType Directory -Path $root | Out-Null
$hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
$names = @('VOXEL_CAVE_WATER_FAST','VOXEL_CAVE_FLOOD','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_AB_OLD')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
$arguments = @('--benchmark-world=hills','--benchmark-seed=2738958700','--benchmark-scale=half',
    '--benchmark-terrain-camera=31.75,80.25,-57.9,0.0,0.3',
    '--benchmark-weather=clear','--benchmark-review-time=0.5','--benchmark-view-chunks=6',
    '--benchmark-width=1280','--benchmark-height=720','--benchmark-cloud-divisor=3',
    '--benchmark-warmup=20',"--benchmark-duration=$Seconds",'--benchmark-capture-frames',
    '--benchmark-capture-interval=2','--benchmark-capture-hud')
try {
    [Environment]::SetEnvironmentVariable('VOXEL_AB_OLD',$null,'Process')
    $env:VOXEL_CAVE_FLOOD = '1'
    foreach ($round in 1..$Rounds) {
        # Reverse alternate rounds so warm caches do not always favour one side.
        $modes = if ($round % 2) { @(0,1) } else { @(1,0) }
        foreach ($mode in $modes) {
            $dir = Join-Path $root "$round-$mode"
            New-Item -ItemType Directory -Path $dir | Out-Null
            $env:VOXEL_CAVE_WATER_FAST = "$mode"
            $env:VOXEL_CAPTURE_DIR = $dir
            $env:VOXEL_SAVE_DIR = Join-Path $dir 'disposable-saves'
            @(Get-Process -Name cargo,rustc,terrain,minecraft_style_voxel_prototype,review -ErrorAction SilentlyContinue |
                Select-Object ProcessName,Id,CPU) | ConvertTo-Json | Set-Content -LiteralPath "$dir/concurrent-processes.json"
            $p = Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo `
                -WindowStyle Hidden -PassThru -RedirectStandardOutput "$dir/native.out" -RedirectStandardError "$dir/native.err"
            try {
                if (-not $p.WaitForExit(180000)) { throw 'Cave-water review timed out' }
                if ($p.ExitCode -ne 0) { throw "Cave-water review exited $($p.ExitCode): $dir" }
            } finally {
                if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
            }
            if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash -ne $hash) { throw 'Executable changed during review' }
            if (-not (Select-String -LiteralPath "$dir/native.err" -Pattern '\[cave-flood\].*placed=[1-9]' -Quiet)) {
                throw "No water was poured: $dir"
            }
        }
    }
    [pscustomobject]@{ executable=$exe; sha256=$hash; rounds=$Rounds; seconds=$Seconds;
        baseline='CAVE_WATER_FAST=0'; candidate='CAVE_WATER_FAST=1';
        note='Same binary; compares sheltered-water shading only. Inspect geometry and concurrent load before interpreting timings.'
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'review.json')
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
}
