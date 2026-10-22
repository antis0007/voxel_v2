param(
    [ValidateSet('poses','combat')][string]$Mode='poses',
    [ValidateSet(0,2,3,5,9)][int]$Kind=0,
    [string]$Label='arcane-enemies', [int]$Seconds=12,
    [double]$CaptureHz=4, [string]$Executable='target/visual-direction/arcane-threats-v4.exe'
)
$ErrorActionPreference='Stop'
$names=@('VOXEL_REVIEW_ENEMIES','VOXEL_ENEMY_REVIEW_KIND','VOXEL_REVIEW_INPUT_ISOLATED')
$prior=@{}; foreach($name in $names){$prior[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_REVIEW_ENEMIES=$Mode
    $env:VOXEL_ENEMY_REVIEW_KIND=if($Kind -gt 0){[string]$Kind}else{$null}
    $env:VOXEL_REVIEW_INPUT_ISOLATED='1'
    & "$PSScriptRoot/Review-VisualDirection.ps1" -World superflat -Case daylight -Label "$Label-$Mode-$Kind" `
        -Width 1280 -Height 720 -Seconds $Seconds -CaptureHz $CaptureHz -Executable $Executable -HideHud:($Mode -eq 'poses')
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$prior[$name])}}
