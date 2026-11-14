[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Baseline,
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$SourceDirectory = (Split-Path $PSScriptRoot -Parent),
    [ValidateRange(1,9)][int]$Runs = 3,
    [ValidateRange(0,100)][double]$MaxRegressionPercent = 8,
    [switch]$AllowIdenticalBinary
)
$ErrorActionPreference = 'Stop'
$Baseline = (Resolve-Path -LiteralPath $Baseline).Path
$Candidate = (Resolve-Path -LiteralPath $Candidate).Path
$SourceDirectory = (Resolve-Path -LiteralPath $SourceDirectory).Path
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new evidence directory; prior results are never overwritten.' }
$OutputDirectory = (New-Item -ItemType Directory -Path $OutputDirectory).FullName
$records = [System.Collections.Generic.List[object]]::new()
$contenders = [System.Collections.Generic.List[object]]::new()
function Get-Contenders([int]$OwnPid = 0) {
    @(Get-CimInstance Win32_Process | Where-Object {
        $_.ProcessId -ne $OwnPid -and $_.ProcessId -ne $PID -and
        ($_.Name -match '^(cargo|rustc|rust-lld|link|minecraft_style_voxel_prototype(?:-[\w-]+)?|engine_[a-z_]+-[0-9a-f]+|.*-tests(?:-[\w-]+)?|axiom)\.exe$' -or
         $_.CommandLine -match '--multiplayer-review|--benchmark|coherent_frame_dense_sparse_cost')
    } | Select-Object ProcessId,Name,CommandLine)
}
$initial = @(Get-Contenders)
ConvertTo-Json -InputObject $initial -Depth 4 | Set-Content (Join-Path $OutputDirectory 'initial-processes.json')
if ($initial.Count) { throw 'A build or gameplay process is active. No benchmark started; coordinate a quiet lane.' }
$identities = @{
    baseline = @{ path=$Baseline; sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $Baseline).Hash }
    candidate = @{ path=$Candidate; sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $Candidate).Hash }
    source = $SourceDirectory; runs=$Runs; maxRegressionPercent=$MaxRegressionPercent
    startedUtc = [DateTime]::UtcNow.ToString('o'); os = [Environment]::OSVersion.VersionString
    runnerSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath).Hash
    saveDirectory = (Join-Path $OutputDirectory 'test-saves')
}
$identities | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'identity.json')
if (-not $AllowIdenticalBinary -and $identities.baseline.sha256 -eq $identities.candidate.sha256) {
    throw 'Baseline and candidate have the same SHA-256. Rebuild, or use -AllowIdenticalBinary only for an intentional A/A control.'
}
for ($i = 0; $i -lt $Runs; $i++) {
    $order = if ($i % 2) { @('candidate','baseline') } else { @('baseline','candidate') }
    foreach ($role in $order) {
        $exe = if ($role -eq 'baseline') { $Baseline } else { $Candidate }
        $stem = Join-Path $OutputDirectory "$role-$i"
        $previousSaveDirectory = $env:VOXEL_SAVE_DIR
        try {
            $env:VOXEL_SAVE_DIR = $identities.saveDirectory
            $p = Start-Process -FilePath $exe -ArgumentList @('coherent_frame_dense_sparse_cost','--ignored','--nocapture','--test-threads=1') -WorkingDirectory $SourceDirectory -RedirectStandardOutput "$stem.out.log" -RedirectStandardError "$stem.err.log" -WindowStyle Hidden -PassThru
        } finally {
            if ($null -eq $previousSaveDirectory) { Remove-Item Env:VOXEL_SAVE_DIR -ErrorAction SilentlyContinue }
            else { $env:VOXEL_SAVE_DIR = $previousSaveDirectory }
        }
        do {
            foreach ($other in (Get-Contenders -OwnPid $p.Id)) { $contenders.Add($other) }
            Start-Sleep -Milliseconds 300
            $p.Refresh()
        } while (-not $p.HasExited)
        $p.WaitForExit()
        if ($p.ExitCode -ne 0) { throw "$role benchmark exited $($p.ExitCode); see $stem logs." }
        $text = (Get-Content -LiteralPath "$stem.out.log" -Raw) + (Get-Content -LiteralPath "$stem.err.log" -Raw)
        $matches = [regex]::Matches($text, '\[coherent-frame-cost\] case=(\S+) cells=(\d+) machines=(\d+) metric=(\S+) median=([0-9.]+) max=([0-9.]+) unit=(\S+) bytes=(\d+) staged_peak=(\d+)')
        if ($matches.Count -ne 30) { throw "Expected 30 measurement rows, got $($matches.Count) from $role. The executable may lack the ignored benchmark." }
        foreach ($m in $matches) {
            $records.Add([pscustomobject]@{
                role=$role; run=$i; case=$m.Groups[1].Value; cells=[int]$m.Groups[2].Value
                machines=[int]$m.Groups[3].Value; metric=$m.Groups[4].Value
                median=[double]::Parse($m.Groups[5].Value,[cultureinfo]::InvariantCulture)
                maximum=[double]::Parse($m.Groups[6].Value,[cultureinfo]::InvariantCulture)
                unit=$m.Groups[7].Value; bytes=[int]$m.Groups[8].Value; stagedPeak=[int]$m.Groups[9].Value
            })
        }
    }
}
$records | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'measurements.json')
ConvertTo-Json -InputObject $contenders.ToArray() -Depth 4 | Set-Content (Join-Path $OutputDirectory 'contenders.json')
$summary = foreach ($case in @('dense4k','dense64k','dense256k','sparse64','sparse512')) {
    foreach ($metric in @('encode','decode','group','final_poll_commit')) {
        $before = @($records | Where-Object {$_.role -eq 'baseline' -and $_.case -eq $case -and $_.metric -eq $metric} | Sort-Object median)
        $after = @($records | Where-Object {$_.role -eq 'candidate' -and $_.case -eq $case -and $_.metric -eq $metric} | Sort-Object median)
        $b=$before[[int][math]::Floor($Runs/2)].median; $a=$after[[int][math]::Floor($Runs/2)].median
        [pscustomobject]@{case=$case; metric=$metric; baselineMs=$b; candidateMs=$a; changePercent=100*($a/$b-1); exceedsLimit=$a -gt $b*(1+$MaxRegressionPercent/100)}
    }
}
$status = if ($contenders.Count) {'contended'} elseif (@($summary | Where-Object exceedsLimit).Count) {'regression'} else {'passed'}
@{status=$status; summary=@($summary)} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'summary.json')
$summary | Format-Table -AutoSize
Write-Output "Status: $status. CPU test timings only; not rendered gameplay or WAN acceptance."
if ($status -eq 'contended') { exit 3 }
if ($status -eq 'regression') { exit 1 }
