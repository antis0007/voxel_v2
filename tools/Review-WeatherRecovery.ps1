param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OutputRoot,
    [ValidateSet('Transition','Lighting')][string]$Views = 'Transition',
    [int]$Warmup = 25,
    [int]$Duration = 8,
    [string[]]$Labels = @()
)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $OutputRoot) { throw "Output already exists: $OutputRoot" }
$root = (New-Item -ItemType Directory -Path $OutputRoot).FullName
$poses = if ($Views -eq 'Transition') {
    @(
        @('2000', '96,45,2000,0.6,-0.45', '0.75'),
        @('4000', '96,45,4000,0.6,-0.45', '0.75'),
        @('6000', '96,45,6000,0.6,-0.45', '0.75'),
        @('9000', '96,45,9000,0.6,-0.45', '0.75'),
        @('orbit', '0,0,60000,0,-1.5', '0.75'),
        @('edge', '0,0,90000,0,-0.9', '0.75')
    )
} else {
    @(
        @('dusk', '96,45,4000,0.6,-0.45', '0.5', '0'),
        @('night-day0', '96,45,4000,0.6,-0.45', '0.25', '0'),
        @('night-day2', '96,45,4000,0.6,-0.45', '0.25', '2')
    )
}
if ($Labels.Count) { $poses = @($poses | Where-Object { $_[0] -in $Labels }) }
$names = @('VOXEL_CAPTURE_DIR', 'VOXEL_SAVE_DIR', 'VOXEL_GLOBE', 'VOXEL_REVIEW_WAND')
$saved = @{}
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:VOXEL_GLOBE = '1'
    $env:VOXEL_REVIEW_WAND = '2'
    $hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    foreach ($pose in $poses) {
        $out = (New-Item -ItemType Directory -Path (Join-Path $root $pose[0])).FullName
        $env:VOXEL_CAPTURE_DIR = $out
        $env:VOXEL_SAVE_DIR = Join-Path $out 'save'
        $day = if ($pose.Count -gt 3) { $pose[3] } else { '0' }
        $arguments = @('--benchmark-world=planet', '--benchmark-seed=2738958700',
            '--benchmark-generator-version=126',
            "--benchmark-review-day=$day",
            '--benchmark-weather=overcast', "--benchmark-review-time=$($pose[2])",
            '--benchmark-view-chunks=4', '--benchmark-cloud-divisor=3',
            '--benchmark-width=1280', '--benchmark-height=720',
            "--benchmark-terrain-camera=$($pose[1])", "--benchmark-warmup=$Warmup",
            "--benchmark-duration=$Duration", '--benchmark-capture-frames', '--benchmark-capture-interval=2')
        $process = Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput (Join-Path $out 'stdout.log') `
            -RedirectStandardError (Join-Path $out 'stderr.log')
        $finished = $false
        for ($attempt = 0; $attempt -lt 4; $attempt++) {
            if ($process.WaitForExit(60000)) { $finished = $true; break }
        }
        if (!$finished) { $process.Kill(); throw "Capture timed out: $($pose[0])" }
        $process.Refresh()
        $receipt = @{ executable = $exe; sha256 = $hash; arguments = $arguments;
            exitCode = $process.ExitCode; capturedAt = (Get-Date -Format o) }
        $receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $out 'manifest.json')
        if ($process.ExitCode -ne 0 -or !(Get-ChildItem -LiteralPath $out -Filter '*.png')) {
            throw "Capture failed: $($pose[0]); see $out"
        }
        Write-Output "Captured $($pose[0]): $out"
    }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
}
