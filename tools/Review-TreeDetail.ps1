param(
    [string]$Binary = "$PSScriptRoot\..\target\release-fast\minecraft_style_voxel_prototype.exe",
    [string]$Output = "$PSScriptRoot\..\target\tree-detail-review"
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot\..").Path
$source = (Resolve-Path -LiteralPath $Binary).Path
$frozen = Join-Path (Split-Path $source) 'tree-detail-reviewed.exe'
Copy-Item -LiteralPath $source -Destination $frozen
New-Item -ItemType Directory -Force -Path $Output | Out-Null
$captureRoot = (Resolve-Path -LiteralPath $Output).Path
Get-FileHash -LiteralPath $frozen -Algorithm SHA256 | Format-List | Out-File (Join-Path $captureRoot 'binary.txt')
$previousCapture = $env:VOXEL_CAPTURE_DIR
Push-Location $repo
try {
    foreach ($case in @(
        @{ Name = 'before-126-quarter'; Version = 126; Scale = 'quarter' },
        @{ Name = 'after-127-quarter'; Version = 127; Scale = 'quarter' },
        @{ Name = 'after-127-1m'; Version = 127; Scale = '1' }
    )) {
        $env:VOXEL_CAPTURE_DIR = Join-Path $captureRoot $case.Name
        $arguments = @('--benchmark', '--benchmark-world=hills', '--benchmark-seed=2738958700',
            "--benchmark-generator-version=$($case.Version)", "--benchmark-scale=$($case.Scale)",
            '--benchmark-terrain-camera=19.75,70.75,9.5,0,-0.08',
            '--benchmark-motion=static', '--benchmark-review-time=0.35',
            '--benchmark-weather=clear', '--benchmark-width=1280', '--benchmark-height=720',
            '--benchmark-warmup=10', '--benchmark-duration=4', '--benchmark-capture-frames',
            '--benchmark-capture-interval=1')
        & $frozen @arguments *> (Join-Path $captureRoot "$($case.Name).log")
        if ($LASTEXITCODE -ne 0) { throw "Tree review $($case.Name) exited with $LASTEXITCODE" }
    }
}
finally {
    $env:VOXEL_CAPTURE_DIR = $previousCapture
    Pop-Location
}
Write-Output $captureRoot
