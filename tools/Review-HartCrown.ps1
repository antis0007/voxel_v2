param(
    [ValidateSet('Intact','Cut','Combat','Manual')] [string]$Case='Intact',
    [string]$Executable='target/release-fast/hart_crown_review.exe',
    [string]$Label='crown',
    [int]$Seconds=22,
    [ValidateRange(1,16)] [int]$ViewChunks=4,
    [ValidateRange(1,20)] [int]$CaptureHz=15
)
$ErrorActionPreference='Stop'
$names=@('VOXEL_HART_CROWN_REVIEW','VOXEL_HART_CROWN_CUT','VOXEL_HART_MANUAL','VOXEL_HART_CROWN_RETARGET')
$previous=@{}
foreach ($name in $names) { $previous[$name]=[Environment]::GetEnvironmentVariable($name) }
try {
    $env:VOXEL_HART_CROWN_REVIEW='1'
    $env:VOXEL_HART_CROWN_CUT=if ($Case -eq 'Cut') { '1' } else { $null }
    $env:VOXEL_HART_MANUAL=if ($Case -eq 'Manual') { '1' } else { $null }
    $env:VOXEL_HART_CROWN_RETARGET=if ($Case -in @('Intact','Cut')) { '1' } else { $null }
    $mode=if ($Case -eq 'Manual') { 'Manual' } elseif ($Case -eq 'Combat') { 'Duel' } else { 'Orbit' }
    if ($Case -eq 'Manual' -and -not $PSBoundParameters.ContainsKey('Seconds')) { $Seconds=600 }
    & (Join-Path $PSScriptRoot 'Review-PrismHart.ps1') -Mode $mode -Variant 0 -Tier 0 `
        -Executable $Executable -Label "$Label-$Case" -Seconds $Seconds -CaptureHz $CaptureHz -Width 1280 -Height 720 -Warmup 0 -ViewChunks $ViewChunks
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$previous[$name]) }
}
