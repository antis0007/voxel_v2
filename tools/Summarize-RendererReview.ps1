param([Parameter(Mandatory=$true)][string[]]$Paths)
$ErrorActionPreference='Stop'
$culture=[Globalization.CultureInfo]::InvariantCulture
function Numbers($rows,[string]$column) {
    @($rows | ForEach-Object { [double]::Parse($_.$column,$culture) })
}
function Mean($values) {
    if(-not $values.Count){return $null}
    [Math]::Round(($values | Measure-Object -Average).Average,4)
}
function Percentile($values,[double]$fraction) {
    if(-not $values.Count){return $null}
    $sorted=@($values | Sort-Object)
    $index=[int][Math]::Round(($sorted.Count-1)*$fraction,[MidpointRounding]::AwayFromZero)
    [Math]::Round($sorted[$index],4)
}
foreach($path in $Paths){
    $root=(Resolve-Path -LiteralPath $path).Path
    $performanceAccepted=$null
    $acceptanceReason=$null
    $acceptancePath=Join-Path $root 'acceptance.json'
    if(Test-Path -LiteralPath $acceptancePath){
        $acceptance=Get-Content -Raw -LiteralPath $acceptancePath | ConvertFrom-Json
        $decision=$acceptance.PSObject.Properties['accepted_for_performance']
        if($null -ne $decision){
            $performanceAccepted=$decision.Value
            $acceptanceReason=$acceptance.reason
            if($performanceAccepted -isnot [bool]){
                throw "Invalid performance acceptance decision: $acceptancePath"
            }
            if(-not $performanceAccepted){
                throw "Performance evidence rejected: $root ($acceptanceReason)"
            }
        }
    }
    $summary=Get-Content -LiteralPath (Join-Path $root 'summary.txt')
    if(-not ($summary -match 'terrain_draws=[1-9]')){
        throw "No rendered terrain: reject $root"
    }
    $manifest=Get-Content -Raw -LiteralPath (Join-Path $root 'manifest.json') | ConvertFrom-Json
    $rows=@(Import-Csv -LiteralPath (Join-Path $root 'frames.csv'))
    if(-not $rows.Count){throw "No frame samples: $root"}
    $times=Numbers $rows 'total_ms'
    $hasProvenance=$rows[0].PSObject.Properties.Name -contains 'gpu_sample_count'
    $gpu=@();$duplicates=0;$outsideWindow=0;$configurationCrossovers=0
    if($hasProvenance){
        $ids=[Collections.Generic.HashSet[UInt64]]::new()
        $frameIds=@($rows | ForEach-Object {[UInt64]$_.render_frame_id} | Where-Object {$_ -ne 0})
        $range=$frameIds | Measure-Object -Minimum -Maximum
        $gpu=@($rows | Where-Object {
            if([int]$_.gpu_sample_count -eq 0){return $false}
            if([UInt64]$_.gpu_frame_id -gt [UInt64]$_.render_frame_id){throw "GPU result from future frame: $root"}
            if(-not $ids.Add([UInt64]$_.gpu_frame_id)){$script:duplicates++;return $false}
            if([UInt64]$_.gpu_frame_id -lt $range.Minimum -or [UInt64]$_.gpu_frame_id -gt $range.Maximum){
                $script:outsideWindow++;return $false
            }
            if($_.gpu_config_id -ne $_.render_config_id){$script:configurationCrossovers++}
            return $true
        })
        if($duplicates){throw "Duplicate GPU sample IDs ($duplicates): $root"}
    }
    $gpuTimes=Numbers $gpu 'gpu_pass_ms'
    $horizonTimes=@()
    $hasHorizonTiming=$rows[0].PSObject.Properties.Name -contains 'horizon_ms'
    if($hasHorizonTiming){$horizonTimes=Numbers $rows 'horizon_ms'}
    $streamFrames=$null;$peakUpload=$null;$gpuResident=$null
    if($rows[0].PSObject.Properties.Name -contains 'chunk_upload_bytes'){
        $streamFrames=@($rows | Where-Object {[long]$_.chunk_upload_bytes -gt 0 -or [int]$_.worker_pending -gt 0}).Count
        $peakUpload=($rows | ForEach-Object {[long]$_.chunk_upload_bytes} | Measure-Object -Maximum).Maximum
        $gpuResident=($rows | ForEach-Object {[long]$_.gpu_resident_bytes} | Measure-Object -Maximum).Maximum
    }
    [pscustomobject][ordered]@{
        path=$root;executable_sha256=$manifest.sha256;capture_readback=$manifest.capture_readback
        performance_accepted=$performanceAccepted;performance_acceptance_reason=$acceptanceReason
        arguments=($manifest.arguments -join ' ');cpu_frames=$rows.Count
        measured_seconds=[Math]::Round(($times | Measure-Object -Sum).Sum/1000,3)
        frame_mean_ms=(Mean $times);frame_p50_ms=(Percentile $times 0.5)
        frame_p95_ms=(Percentile $times 0.95);frame_p99_ms=(Percentile $times 0.99)
        frame_max_ms=(Percentile $times 1);gpu_provenance=$hasProvenance
        gpu_samples=$gpu.Count;gpu_warmup_samples_excluded=$outsideWindow
        gpu_configuration_crossovers=$configurationCrossovers
        gpu_mean_ms=(Mean $gpuTimes);gpu_p95_ms=(Percentile $gpuTimes 0.95)
        gpu_cloud_mean_ms=(Mean (Numbers $gpu 'cloud_ms'))
        gpu_terrain_mean_ms=(Mean (Numbers $gpu 'terrain_ms'))
        horizon_cpu_timing=$hasHorizonTiming
        horizon_cpu_mean_ms=(Mean $horizonTimes)
        horizon_cpu_p95_ms=(Percentile $horizonTimes 0.95)
        horizon_cpu_max_ms=(Percentile $horizonTimes 1)
        horizon_cpu_scope='Coverage construction and full horizon update, including validation, worker dispatch and incremental upload submission. Excludes asynchronous worker execution and GPU execution.'
        streaming_frames=$streamFrames;peak_chunk_upload_bytes=$peakUpload
        peak_resident_chunk_gpu_bytes=$gpuResident
        gpu_scope='Existing timed passes; excludes HDR resolve/present shader, egui overlay, uploads, acquisition and presentation waits. Legacy CSVs have no independent GPU samples.'
    }
}
