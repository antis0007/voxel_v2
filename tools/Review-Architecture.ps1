# Native fixed-seed gate. Each image has its own executable hash and camera receipt.
param([Parameter(Mandatory=$true)][string]$Executable,
      [Parameter(Mandatory=$true)][string]$Label,
      [string]$Scale='0.25', [string]$OutputBase='', [int]$StartAt=0, [int]$Generator=61)
$ErrorActionPreference='Stop'
$views=@(
    @('front','workshop-front','none',0.5),
    @('back','workshop-back','none',0.5),
    @('left','workshop-left','none',0.5),
    @('right','workshop-right','none',0.5),
    @('front-quarter','workshop','none',0.5),
    @('rear-quarter','workshop-rear-quarter','none',0.5),
    @('roof','workshop-roof','none',0.5),
    @('exit','workshop-exit','none',0.5),
    @('work','workshop-inside','none',0.5),
    @('clay','workshop-roof','clay',0.5),
    @('materials','workshop-roof','material-id',0.5),
    @('night','workshop-exit','none',0.0)
)
$failures=@()
foreach($view in ($views | Select-Object -Skip $StartAt)){
    try {
    & "$PSScriptRoot/Review-Places.ps1" -Executable $Executable -Label "$Label-$($view[0])" `
        -Mode "place:$($view[1])" -Generator $Generator -Scale $Scale -DebugView $view[2] `
        -TimeOfDay $view[3] -Seconds 2 -SkipHorizon -OutputBase $OutputBase
    } catch { $failures += $view[0]; Write-Warning $_ }
}
if($failures.Count){throw "Incomplete architecture views: $($failures -join ', ')"}
