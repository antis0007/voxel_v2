param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Prefix,
    [string]$Repo = (Split-Path $PSScriptRoot -Parent),
    [string]$EvidenceRoot = ''
)
$ErrorActionPreference='Stop'
if (-not $EvidenceRoot) { $EvidenceRoot = Join-Path $Repo 'target/material-distance-review' }
$cases=@(
    @{ Name='m1'; Scale='1'; Camera='0,-32,2,0,-0.28'; Time=0.5 },
    @{ Name='m05'; Scale='0.5'; Camera='0,-32,2,0,-0.28'; Time=0.5 },
    @{ Name='m025'; Scale='0.25'; Camera='0,-32,2,0,-0.28'; Time=0.5 },
    @{ Name='m025-angle'; Scale='0.25'; Camera='-2,-24,2,0.18,-0.14'; Time=0.5 },
    @{ Name='m025-dusk'; Scale='0.25'; Camera='0,-32,2,0,-0.28'; Time=0.72 }
)
foreach($case in $cases) {
    $label="$Prefix-$($case.Name)"
    $receipt=& "$PSScriptRoot\review-material-distance.ps1" -Executable $Executable -Label $label -Scale $case.Scale -Camera $case.Camera -TimeOfDay $case.Time -Repo $Repo -EvidenceRoot $EvidenceRoot | ConvertFrom-Json
    "$label started process $($receipt.pid)"
    $process=Get-Process -Id $receipt.pid
    $watch=[Diagnostics.Stopwatch]::StartNew()
    while(-not $process.WaitForExit(500)) {
        if($watch.Elapsed.TotalSeconds -gt 240) { Stop-Process -Id $receipt.pid; throw "$label exceeded four minutes" }
    }
    $out=$receipt.evidence
    if(-not (Select-String -LiteralPath (Join-Path $out 'stdout.log') -SimpleMatch '[benchmark] avg_fps=' -Quiet)) { throw "$label incomplete" }
    $summary=Select-String -LiteralPath (Join-Path $out 'stdout.log') -Pattern '\[benchmark\]' | ForEach-Object Line
    $summary | Set-Content (Join-Path $out 'summary.txt')
    $summary | Select-Object -Last 5
}
