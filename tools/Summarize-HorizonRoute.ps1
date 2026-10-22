param([Parameter(Mandatory=$true)][string]$CaseRoot)
$ErrorActionPreference='Stop'
$root=(Resolve-Path -LiteralPath $CaseRoot).Path
$culture=[Globalization.CultureInfo]::InvariantCulture
$gaps=[Collections.Generic.List[object]]::new()
$previous=$null;$open=$null;$seenReady=$false;$frames=0;$failed=0
foreach($row in (Import-Csv -LiteralPath (Join-Path $root 'route-frames.csv'))){
    $frames++
    if($row.render_ok -ne 'true'){$failed++}
    $ready=$row.horizon_ready -eq 'true'
    $point=@([double]::Parse($row.eye_x_m,$culture),[double]::Parse($row.eye_y_m,$culture),[double]::Parse($row.eye_z_m,$culture))
    $seconds=[double]::Parse($row.seconds,$culture)
    if(-not $ready -and $seenReady -and $null -eq $open){
        $jump=[Math]::Sqrt([Math]::Pow($point[0]-$previous.point[0],2)+[Math]::Pow($point[2]-$previous.point[2],2))
        $open=[ordered]@{start_seconds=$seconds;end_seconds=$null;start_frame=[long]$row.frame;end_frame=$null;horizontal_jump_m=$jump;classification=$(if($jump -gt 2048){'explicit_teleport'}else{'ordinary_travel_failure'});eye_m=$point}
    }
    if($ready){
        if($null -ne $open){$open.end_seconds=$seconds;$open.end_frame=[long]$row.frame;$gaps.Add($open);$open=$null}
        $seenReady=$true
    }
    $previous=@{point=$point;seconds=$seconds}
}
if($null -ne $open){$gaps.Add($open)}
$ordinary=@($gaps|Where-Object {$_.classification -eq 'ordinary_travel_failure'})
$report=[ordered]@{
    scope='Actual native horizon_ready/render_ok receipts. This detects loading transitions; it does not prove geometric watertightness, edit/collision atomicity or visual silhouette accuracy.'
    frames=$frames;ever_ready=$seenReady;failed_renders=$failed
    ready_to_loading_transitions=$gaps.Count;ordinary_travel_failures=$ordinary.Count
    loading_intervals=@($gaps.ToArray())
    continuity_receipt_passed=($seenReady -and $failed -eq 0 -and $ordinary.Count -eq 0)
}
$report|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $root 'route-coverage.json')
$report|ConvertTo-Json -Depth 6
