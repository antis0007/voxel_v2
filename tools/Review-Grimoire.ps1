param(
    [ValidateSet('hud','inventory','wands','components','magic')][string]$Screen = 'hud',
    [ValidateSet('daylight','water','clouds','canopy','cave','motion')][string]$Case = 'water',
    [string]$Label = 'grimoire-candidate',
    [int]$Width = 1280,
    [int]$Height = 720,
    [int]$Seconds = 30,
    [double]$CaptureHz = 0.2,
    [ValidateRange(0,127)][int]$SpellId,
    [switch]$Codex,
    [switch]$AllowBusy,
    [switch]$Animate,
    [switch]$LiveClock,
    [string]$Executable = 'target/visual-direction/arcane-threats-v4.exe'
)
$ErrorActionPreference = 'Stop'
$prior = $env:VOXEL_GRIMOIRE_REVIEW
$priorMotion = $env:VOXEL_GRIMOIRE_MOTION_REVIEW
$priorIsolated = $env:VOXEL_REVIEW_INPUT_ISOLATED
$priorSpell = $env:VOXEL_REVIEW_SPELL_CARD
$priorCodex = $env:VOXEL_REVIEW_CODEX
try {
    $env:VOXEL_GRIMOIRE_REVIEW = if ($Screen -eq 'hud') { $null } else { $Screen }
    $env:VOXEL_GRIMOIRE_MOTION_REVIEW = if ($Animate) { '1' } else { $null }
    $env:VOXEL_REVIEW_INPUT_ISOLATED = '1'
    if ($PSBoundParameters.ContainsKey('SpellId')) { $env:VOXEL_REVIEW_SPELL_CARD = [string]$SpellId }
    if ($Codex) { $env:VOXEL_REVIEW_CODEX = '1' }
    & "$PSScriptRoot/Review-VisualDirection.ps1" -Case $Case -Label "$Label-$Screen" `
        -Width $Width -Height $Height -Seconds $Seconds -CaptureHz $CaptureHz -Executable $Executable -LiveClock:$LiveClock -AllowBusy:$AllowBusy
} finally {
    $env:VOXEL_GRIMOIRE_REVIEW = $prior
    $env:VOXEL_GRIMOIRE_MOTION_REVIEW = $priorMotion
    $env:VOXEL_REVIEW_INPUT_ISOLATED = $priorIsolated
    $env:VOXEL_REVIEW_SPELL_CARD = $priorSpell
    $env:VOXEL_REVIEW_CODEX = $priorCodex
}
