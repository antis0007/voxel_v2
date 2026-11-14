param(
    [string]$Executable = 'target/release-fast/minecraft_style_voxel_prototype.exe',
    [string]$Label = ('storm-surge-' + (Get-Date -Format 'yyyyMMdd-HHmmss')),
    [switch]$AllowBusy
)
$ErrorActionPreference = 'Stop'
$vars = @('VOXEL_REVIEW_ENEMIES','VOXEL_ENEMY_REVIEW_SCENE','VOXEL_REVIEW_INPUT_ISOLATED')
$previous = @{}
foreach ($name in $vars) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:VOXEL_REVIEW_ENEMIES = 'duel'
    $env:VOXEL_ENEMY_REVIEW_SCENE = 'storm-surge'
    $env:VOXEL_REVIEW_INPUT_ISOLATED = '1'
    & "$PSScriptRoot/Review-VisualDirection.ps1" -Case daylight -World superflat -Label $Label `
        -Executable $Executable -Width 1280 -Height 720 -Seconds 8 -CaptureHz 2 -ViewChunks 4 `
        -CameraPose '0.5,4.5,80,0,-0.32' -AllowBusy:$AllowBusy
    $root = Join-Path (Split-Path $PSScriptRoot -Parent) "target/visual-direction/$Label/daylight-1280-720"
    $logs = Get-ChildItem -LiteralPath $root -File | Where-Object Extension -eq '.log'
    if (!$logs) { throw "No native review logs in $root" }
    $results = Select-String -LiteralPath $logs.FullName -SimpleMatch '[storm-surge-review]'
    $results | ForEach-Object Line
    if ($results.Line -notcontains '[storm-surge-review] PASS') {
        throw "Storm Surge acceptance receipt missing in $root"
    }
} finally {
    foreach ($name in $vars) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
}
