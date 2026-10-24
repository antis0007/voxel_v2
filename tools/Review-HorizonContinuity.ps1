param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [string]$EvidenceRoot='',
    [ValidateSet('still','route','motion','probe','perf','edits','stress','handoff','world-switch')][string]$Case='still',
    [int]$Seconds=45,
    [ValidateRange(0,120)][int]$WarmupSeconds=4,
    [int]$Width=1280,[int]$Height=720,[int]$ViewChunks=4,
    [ValidateRange(1,64)][int]$GeneratorVersion=25,
    [string]$Camera='0,-120,100,0.4,-0.2',
    [string]$DebugView='', [string]$ExactPatch='',
    [ValidateRange(1,4)][int]$CloudDivisor=3,
    [int]$UploadBytes=8388608,
    [ValidateRange(0,60000)][int]$DelayMilliseconds=0,
    [ValidateRange(0.05,5)][double]$CaptureInterval=1.0,
    [switch]$Video, [switch]$DirectVideo, [switch]$EditInFlight, [switch]$VoxelReference,
    [switch]$NoShadows, [switch]$NoIndirect, [switch]$Physics,
    [switch]$AuditProcesses
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe=(Resolve-Path -LiteralPath $Executable).Path
if($DirectVideo){$Video=$true;$CaptureInterval=0.05}
if($Label -notmatch '^[a-zA-Z0-9_-]+$'){throw 'Label must contain only letters, digits, underscores or hyphens'}
if($Width -lt 320 -or $Height -lt 240 -or $ViewChunks -lt 1 -or $Seconds -lt 1){throw 'Invalid review dimensions, radius or duration'}
if($Video -and $Case -in @('probe','perf')){throw 'Video readback or source probing must be separate from performance measurements'}
if($VoxelReference -and -not $ExactPatch){throw 'VoxelReference requires ExactPatch bounds'}
if($Case -in @('edits','stress','handoff')){$EditInFlight=$true}
$evidenceBase=if($EvidenceRoot){[IO.Path]::GetFullPath($EvidenceRoot)}else{Join-Path $repo 'target/reviews/horizon-continuity'}
$root=Join-Path $evidenceBase $Label
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root -Force | Out-Null
if($Case -in @('route','edits','stress')){$Seconds=[Math]::Max($Seconds,280)}
if($Case -eq 'handoff'){$Seconds=[Math]::Max($Seconds,90)}
if($Case -eq 'world-switch'){$Seconds=[Math]::Max($Seconds,45)}
if($Case -eq 'motion'){$Seconds=[Math]::Max($Seconds,45)}
if($Video -and -not $DirectVideo){$CaptureInterval=[Math]::Max($CaptureInterval,$Seconds/600.0)}
if($Video){
    $drive=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($root))
    $reservation=if($DirectVideo){[long](($Seconds+60)*750000+[Math]::Ceiling(($Seconds+60)/10)*1.5*1024*1024+144*1024*1024)}else{[long]([Math]::Ceiling($Seconds/$CaptureInterval)*1.5*1024*1024+160*1024*1024)}
    if($drive.AvailableFreeSpace -lt $reservation){throw "Insufficient room for bounded native recording: reserve=$reservation free=$($drive.AvailableFreeSpace). Increase CaptureInterval or reclaim redundant task outputs."}
}
$argsList=@('--benchmark-world=hills','--benchmark-seed=2738958700',"--benchmark-generator-version=$GeneratorVersion",'--benchmark-weather=partly',
    "--benchmark-view-chunks=$ViewChunks","--benchmark-width=$Width","--benchmark-height=$Height",
    "--benchmark-terrain-camera=$Camera","--benchmark-warmup=$WarmupSeconds","--benchmark-duration=$Seconds",
    '--benchmark-review-time=0.5','--benchmark-scale=1m','--benchmark-sky-time=12',"--benchmark-cloud-divisor=$CloudDivisor")
if($Case -in @('still','route','motion','edits','stress','handoff','world-switch')){$argsList+='--benchmark-capture-frames'}
if($Video){$argsList+='--benchmark-capture-interval='+$CaptureInterval.ToString('0.######',[Globalization.CultureInfo]::InvariantCulture)}
if($DebugView){$argsList+="--benchmark-debug=$DebugView"}
if($NoShadows){$argsList+='--benchmark-no-shadows'}
if($NoIndirect){$argsList+='--benchmark-no-indirect'}
if($Physics){$argsList+='--benchmark-physics'}
if($Case -eq 'probe'){$argsList+='--horizon-probe-only'}
$vars=@{VOXEL_REVIEW_DUNGEON=$null;VOXEL_CAPTURE_DIR=$root;VOXEL_HORIZON_ROUTE=$null;VOXEL_HORIZON_PROBE=$null;VOXEL_HORIZON_EDITS=$null;
    VOXEL_HORIZON_WORLD_SWITCH=$null;VOXEL_RENDER_TIMING=$null;VOXEL_HORIZON_TRACE=$null;
    VOXEL_SAVE_DIR=(Join-Path $root 'generated-world');
    VOXEL_CAPTURE_VIDEO=$null;VOXEL_CAPTURE_VIDEO_FPS=$null;
    VOXEL_HORIZON_REFERENCE_VOXELS=$null;
    VOXEL_HORIZON_EXACT_PATCH=$ExactPatch;VOXEL_HORIZON_UPLOAD_BYTES="$UploadBytes";VOXEL_HORIZON_DELAY_MS="$DelayMilliseconds";
    VOXEL_TERRAIN_EXPOSURE='1';VOXEL_TERRAIN_LIGHT=$null;VOXEL_TERRAIN_WIND='0';
    VOXEL_TERRAIN_ROUTE=$null;VOXEL_TERRAIN_EDITS=$null;VOXEL_REVIEW_TERRAIN=$null;VOXEL_REVIEW_WAND=$null}
if($Case -in @('route','motion','edits','stress','handoff')){$vars.VOXEL_HORIZON_ROUTE='1'}
if($Case -eq 'world-switch'){$vars.VOXEL_HORIZON_WORLD_SWITCH='1';$vars.VOXEL_RENDER_TIMING='1'}
if($Case -eq 'perf'){$vars.VOXEL_RENDER_TIMING='1'}
if($DirectVideo){$vars.VOXEL_CAPTURE_VIDEO=(Join-Path $root 'traversal.mp4');$vars.VOXEL_CAPTURE_VIDEO_FPS='20'}
if($Case -eq 'probe'){$vars.VOXEL_HORIZON_PROBE='1'}
if($VoxelReference){$vars.VOXEL_HORIZON_REFERENCE_VOXELS='1'}
if($EditInFlight){
    if($Case -notin @('route','edits','stress','handoff')){throw 'EditInFlight requires route/edits/stress; keep steady-state timing separate'}
    $vars.VOXEL_HORIZON_EDITS='1'
}
$previous=@{}
function Set-ReviewEnvironment([string]$Name,$Value){
    if($null -eq $Value){Remove-Item -LiteralPath "Env:$Name" -ErrorAction SilentlyContinue}
    else{[Environment]::SetEnvironmentVariable($Name,[string]$Value)}
}
foreach($name in $vars.Keys){$previous[$name]=[Environment]::GetEnvironmentVariable($name);Set-ReviewEnvironment $name $vars[$name]}
try {
    [ordered]@{utc=[DateTime]::UtcNow.ToString('o');executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;
        arguments=$argsList;environment=$vars;fov_vertical_degrees=70;render_scale=1;world='isolated generated benchmark; no player save';
        requested_case=$Case;quality_note='ViewChunks is resident radius, not a full graphics preset. Read startup log for actual quality settings.';
        evidence_note='Probe candidates require same-camera visibility confirmation. Source columns exclude edits and overhangs. Capture runs are not timing runs.'} |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$root/manifest.json"
    $p=Start-Process -FilePath $exe -ArgumentList $argsList -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$p.Handle
    $processStartUtc=$p.StartTime.ToUniversalTime().ToString('o')
    $peakWorking=0L;$peakPrivate=0L
    $nextProcessAudit=[DateTime]::UtcNow
    $deadline=[DateTime]::UtcNow.AddSeconds($Seconds+200)
    while(-not $p.WaitForExit(500)){
        try{$p.Refresh();$peakWorking=[Math]::Max($peakWorking,$p.WorkingSet64);$peakPrivate=[Math]::Max($peakPrivate,$p.PrivateMemorySize64)}catch{}
        if($AuditProcesses -and [DateTime]::UtcNow -ge $nextProcessAudit){
            $observed=@(Get-Process -ErrorAction SilentlyContinue | Where-Object {
                $_.Id -ne $p.Id -and ($_.ProcessName -match '^(cargo|rustc|rust-analyzer.*)$' -or
                ($_.Path -and $_.Path.StartsWith((Join-Path $repo 'target'),[StringComparison]::OrdinalIgnoreCase)))
            } | Select-Object ProcessName,Id,CPU)
            [ordered]@{utc=[DateTime]::UtcNow.ToString('o');other_compiler_analyzer_or_project_processes=$observed} |
                ConvertTo-Json -Compress -Depth 4 | Add-Content -LiteralPath "$root/process-audit.jsonl"
            $nextProcessAudit=[DateTime]::UtcNow.AddSeconds(1)
        }
        if($Video -and $drive.AvailableFreeSpace -lt 128*1024*1024){
            [ordered]@{invalid=$true;reason='recording reached its disk reserve';process_id=$p.Id;free_bytes=$drive.AvailableFreeSpace} | ConvertTo-Json | Set-Content "$root/recording-failure.json"
            if(-not $p.CloseMainWindow() -or -not $p.WaitForExit(5000)){Stop-Process -Id $p.Id -ErrorAction SilentlyContinue}
            throw 'Owned recording stopped at disk reserve; partial output is not acceptance evidence.'
        }
        if([DateTime]::UtcNow -gt $deadline){Stop-Process -Id $p.Id -ErrorAction SilentlyContinue;throw 'Review exceeded its duration plus 200-second startup allowance'}
    }
    [ordered]@{exit_code=$p.ExitCode;process_id=$p.Id;start_utc=$processStartUtc;end_utc=$p.ExitTime.ToUniversalTime().ToString('o');sampled_peak_working_set_bytes=$peakWorking;sampled_peak_private_bytes=$peakPrivate;sample_interval_ms=500} |
        ConvertTo-Json | Set-Content "$root/process.json"
    if($p.ExitCode -ne 0){throw "Renderer exit $($p.ExitCode): $root"}
    $logs=@("$root/stdout.log","$root/stderr.log")
    Select-String -LiteralPath $logs -Pattern '^\[(benchmark|horizon|horizon-reference|horizon-route|horizon-probe|horizon-edit|camera-review|capture)\]' |
        ForEach-Object {$_.Line} | Set-Content "$root/summary.txt"
    if(Select-String -LiteralPath $logs -Pattern '^\[benchmark\]\s+failed\b'){
        throw 'Benchmark reported failure despite its process exit code; this run is invalid.'
    }
    if($Case -ne 'probe' -and -not (Select-String -LiteralPath $logs -Pattern '^\[benchmark\] avg_fps=')){
        throw 'Missing completed benchmark measurements; this run is invalid.'
    }
    $captures=@(Get-ChildItem -LiteralPath $root -Filter '*.png' -File)
    if($Case -in @('still','route','motion','edits','stress','handoff','world-switch')){
        if($captures.Count -eq 0){throw 'No PNG capture exists; visual evidence is missing.'}
        if(-not (Select-String -LiteralPath $logs -Pattern '^\[capture\] saved ')){
            throw 'Missing saved-capture receipt; visual evidence is incomplete.'
        }
        foreach($capture in $captures){
            $stream=[IO.File]::OpenRead($capture.FullName)
            try {
                $header=[byte[]]::new(8)
                if($stream.Read($header,0,8) -ne 8 -or [BitConverter]::ToString($header).Replace('-','') -ne '89504E470D0A1A0A'){
                    throw "Invalid PNG capture: $($capture.Name)"
                }
            } finally {$stream.Dispose()}
        }
    }
    if($Case -eq 'world-switch'){
        $output=[IO.File]::ReadAllText("$root/stdout.log")
        foreach($receipt in @('[horizon-world-switch] pending=build','[horizon-world-switch] pending=upload','[horizon-world-switch] complete final_seed=2738958702')){
            if(-not $output.Contains($receipt)){throw "Missing actual application world-switch receipt: $receipt"}
        }
        $expected=2738958700L
        foreach($line in $output.Split("`n")){
            if($line -match '\[horizon-world-switch\] pending=\w+ old_seed=\d+ new_seed=(\d+)'){$expected=[long]$Matches[1]}
            if($line -match '^\[horizon\] publish .*source_cfg=.*seed: (\d+)' -and [long]$Matches[1] -ne $expected){throw 'An obsolete world published after the application switched sources'}
        }
    }
    if($ExactPatch -and $Case -ne 'probe'){
        $referenceMarker=if($VoxelReference){'[horizon-reference] resident_voxel_patch '}else{'[horizon-reference] exact_patch_m='}
        $output=[IO.File]::ReadAllText("$root/stdout.log")
        $referenceAt=$output.IndexOf($referenceMarker,[StringComparison]::Ordinal)
        $publishedAt=$output.IndexOf('[horizon] publish built_center=',[Math]::Max(0,$referenceAt),[StringComparison]::Ordinal)
        if($referenceAt -lt 0 -or $publishedAt -lt 0){
            throw 'Requested reference patch was not generated and published; reference evidence is invalid.'
        }
        if($Case -in @('still','route','edits','stress','handoff') -and $output.IndexOf('[capture] saved ',$publishedAt,[StringComparison]::Ordinal) -lt 0){
            throw 'No capture was saved after reference publication; reference evidence is incomplete.'
        }
    }
    if($Case -eq 'probe' -and -not (Select-String -LiteralPath "$root/stderr.log" -SimpleMatch '[horizon-probe] complete')){
        throw 'Probe did not finish; increase Seconds. Partial files are not acceptance evidence.'
    }
    if($Case -eq 'probe'){
        Import-Csv -LiteralPath "$root/source-proxy.csv" | Group-Object region,footprint_m | ForEach-Object {
            $rows=@($_.Group)
            $errors=@($rows | ForEach-Object {[Math]::Abs([double]::Parse($_.height_error_m,[Globalization.CultureInfo]::InvariantCulture))})
            [pscustomobject]@{region=$rows[0].region;footprint_m=$rows[0].footprint_m;samples=$rows.Count;
                mean_absolute_height_error_m=($errors | Measure-Object -Average).Average;
                max_absolute_height_error_m=($errors | Measure-Object -Maximum).Maximum;
                material_mismatch_count=@($rows | Where-Object {$_.source_material -ne $_.proxy_material}).Count;
                wet_mismatch_count=@($rows | Where-Object {$_.source_wet -ne $_.proxy_wet}).Count;
                order_failure_count=@($rows | Where-Object {$_.order_stable -ne 'true'}).Count}
        } | Export-Csv -NoTypeInformation -LiteralPath "$root/probe-summary.csv"
    }
    if($Case -in @('route','edits','stress') -and -not (Select-String -LiteralPath "$root/stderr.log" -SimpleMatch '[horizon-route] phase=9')){
        throw 'Route did not finish or executable lacks the route hook.'
    }
    if($EditInFlight -and $Case -ne 'handoff' -and -not (Select-String -LiteralPath "$root/stderr.log" -SimpleMatch '[horizon-edit] stage=2')){
        throw 'Edit restore did not finish; edit acceptance is incomplete.'
    }
    if($Case -eq 'handoff'){
        if(-not (Select-String -LiteralPath "$root/stderr.log" -SimpleMatch '[horizon-route] phase=3') -or
            -not (Select-String -LiteralPath "$root/stderr.log" -SimpleMatch '[horizon-edit] stage=1')){
            throw 'Shore handoff canary did not reach approach/retreat with excavation.'
        }
    }
    if($Video -and -not $DirectVideo){
        $frames=@(Get-ChildItem -LiteralPath $root -Filter 'bench-*.png' | Sort-Object {[int]($_.BaseName -replace 'bench-','')})
        if($frames.Count -lt 2){throw 'Fewer than two frames; no recording produced'}
        $concat=[Collections.Generic.List[string]]::new()
        for($i=0;$i -lt $frames.Count;$i++){
            $concat.Add("file '$($frames[$i].Name)'")
            $duration=$CaptureInterval
            if($i+1 -lt $frames.Count){$duration=([int]($frames[$i+1].BaseName -replace 'bench-','')-[int]($frames[$i].BaseName -replace 'bench-',''))*$CaptureInterval}
            $concat.Add('duration '+$duration.ToString('0.###',[Globalization.CultureInfo]::InvariantCulture))
        }
        $concat.Add("file '$($frames[-1].Name)'")
        $concat | Set-Content -Encoding ascii "$root/frames.txt"
        & ffmpeg -hide_banner -loglevel error -f concat -safe 0 -i "$root/frames.txt" -fps_mode vfr -c:v libx264 -crf 18 -pix_fmt yuv420p "$root/traversal.mp4"
        if($LASTEXITCODE -ne 0){throw 'Video encoding failed'}
    }
    if($DirectVideo){
        $receipt=Get-Content -LiteralPath "$root/capture-video.json" -Raw | ConvertFrom-Json
        if(-not $receipt.finalized -or $receipt.error -or $receipt.exit_code -ne 0){throw 'Native video encoder did not finalize successfully'}
        $info=& ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=width,height,nb_read_frames,avg_frame_rate -show_entries format=duration -of json "$root/traversal.mp4"
        if($LASTEXITCODE -ne 0){throw 'Native motion recording could not be decoded'}
        $decoded=$info | ConvertFrom-Json
        $stream=$decoded.streams[0]
        $expected=[long]$receipt.unique_frames+[long]$receipt.repeated_frames
        if($expected -lt 2 -or [long]$stream.nb_read_frames -ne $expected -or $expected -ne ([long]$receipt.last_index-[long]$receipt.first_index+1)){throw 'Native recording frame counts disagree with the readback receipt'}
        if([int]$stream.width -ne $Width -or [int]$stream.height -ne $Height -or [int]$receipt.width -ne $Width -or [int]$receipt.height -ne $Height){throw 'Native recording dimensions disagree with the requested view'}
        $rate="$($stream.avg_frame_rate)".Split('/')
        $fps=[double]$rate[0]/[double]$rate[1]
        $duration=[double]::Parse([string]$decoded.format.duration,[Globalization.CultureInfo]::InvariantCulture)
        if([Math]::Abs($fps-[double]$receipt.fps) -gt 0.001 -or [Math]::Abs($duration-$expected/$fps) -gt 1.0/$fps+0.001){throw 'Native recording chronology disagrees with the readback receipt'}
        [ordered]@{native=$receipt;decoded=$decoded;sha256=(Get-FileHash -LiteralPath "$root/traversal.mp4").Hash;note='Actual WGPU readbacks. Missing intervals may hold the preceding real frame; unique/repeated counts are in the native receipt.'} | ConvertTo-Json -Depth 7 | Set-Content "$root/video-validation.json"
    }
    [ordered]@{runner_completed=$true;capture_count=$captures.Count;reference_requested=[bool]$ExactPatch;
        voxel_reference=[bool]$VoxelReference;visual_acceptance='requires image and motion review'} |
        ConvertTo-Json | Set-Content "$root/acceptance.json"
    Write-Output $root
} catch {
    [ordered]@{runner_completed=$false;reason=$_.Exception.Message;visual_acceptance='invalid or incomplete evidence'} |
        ConvertTo-Json | Set-Content "$root/acceptance.json"
    throw
} finally {foreach($name in $previous.Keys){Set-ReviewEnvironment $name $previous[$name]}}
