# Same-seed, same-mode captures of generated places across generator versions.
# Modes (crates/app/src/dungeon_review.rs): complex:<k> for the complex nearest
# (300, 0), undercrown:<room name> for an Undercrown room. Run once with
# -Generator 31 and once with -Generator 32 for a before/after pair.
param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [Parameter(Mandatory=$true)][string]$Mode,
    [uint32]$Generator=32,
    [uint32]$Seed=424242,
    [ValidateSet('1','0.5','0.25')][string]$Scale='1',
    [double]$TimeOfDay=0.5,
    [string]$DebugView='none',
    [ValidateSet('hills','manyfold')][string]$World='hills',
    [switch]$SkipHorizon,
    [switch]$Physics,
    [string]$Design='',
    [string]$OutputBase='',
    [int]$Seconds=24,
    [int]$MaxWallSeconds=180
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=if($OutputBase){Join-Path $OutputBase $Label}else{Join-Path $repo "target/place-review/$Label"}
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$time=$TimeOfDay.ToString([Globalization.CultureInfo]::InvariantCulture)
$arguments=@("--benchmark-world=$World","--benchmark-seed=$Seed","--benchmark-generator-version=$Generator",
    "--benchmark-scale=$Scale",'--benchmark-weather=clear',"--benchmark-review-time=$time",
    '--benchmark-view-chunks=4','--benchmark-width=1920','--benchmark-height=1080',
    '--benchmark-terrain-camera=0,0,1,0,0','--benchmark-warmup=14',
    "--benchmark-duration=$Seconds", "--benchmark-debug=$DebugView",'--benchmark-capture-frames')
if($Physics){$arguments += '--benchmark-physics'}
if($Design){
    $designPath=(Resolve-Path -LiteralPath $Design).Path
    $arguments += ('"--anchor-studio-pack=' + $designPath + '"')
    New-Item -ItemType Directory -Path (Join-Path $root 'generated-world') | Out-Null
}
$vars=@{VOXEL_REVIEW_DUNGEON=$Mode;VOXEL_CAPTURE_DIR=$root;
    VOXEL_SAVE_DIR=(Join-Path $root 'generated-world');VOXEL_REVIEW_WAND=$null;
    VOXEL_TERRAIN_ROUTE=$null;VOXEL_TERRAIN_EDITS=$null;VOXEL_TERRAIN_LIGHT=$null;
    VOXEL_TERRAIN_EXPOSURE=$null;VOXEL_REVIEW_ENEMIES=$null;VOXEL_HART_ORBIT=$null;
    VOXEL_REVIEW_SKIP_CLOUDS=$null;VOXEL_REVIEW_SKIP_HORIZON=$(if($SkipHorizon){'1'}else{$null});
    VOXEL_HORIZON_ROUTE=$null;VOXEL_HORIZON_EDITS=$null;VOXEL_HORIZON_SOURCE=$null;
    VOXEL_CAPTURE_VIDEO=$null;VOXEL_RENDER_TIMING=$null}
$previous=@{}
$process=$null
try {
    foreach($name in $vars.Keys){
        $previous[$name]=[Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name,$vars[$name])
    }
    [ordered]@{utc=[DateTime]::UtcNow.ToString('o');executable=$exe;
        sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;environment=$vars;
        scope='Generated place render review; camera only, not traversal or performance'} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath "$root/manifest.json"
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null=$process.Handle
    $deadline=[DateTime]::UtcNow.AddSeconds($MaxWallSeconds)
    $capturedStop=$false
    while(-not $process.WaitForExit(500) -and [DateTime]::UtcNow -lt $deadline){
        # A diagnostic without the horizon can finish while an unused horizon
        # worker is still building. Stop only this owned process after BOTH
        # a capture and the final benchmark report are on disk. Physics captures
        # can retain pending streaming work; the frame still needs visual review.
        $framePresent = (Test-Path -LiteralPath "$root/camera.png") -or
            (Test-Path -LiteralPath "$root/bench-000.png")
        if($SkipHorizon -and $framePresent -and
            (Select-String -LiteralPath "$root/stdout.log" -Pattern '^\[benchmark\] cloud_history' -Quiet)){
            try {$process.Kill();$process.WaitForExit();$capturedStop=$true;break}
            catch { $SkipHorizon=$false } # Restricted hosts can require natural exit.
        }
    }
    if(-not $process.HasExited){
        $process.Kill();$process.WaitForExit()
        throw "Place review exceeded ${MaxWallSeconds}s wall limit: $root"
    }
    if($process.ExitCode -ne 0 -and -not $capturedStop){throw "Place review exited $($process.ExitCode): $root"}
    if($capturedStop){'Stopped after capture and final report; unused horizon worker cleanup.' | Set-Content "$root/capture-stop.txt"}
    $receipt=Select-String -LiteralPath "$root/stdout.log" -Pattern '^\[dungeon-review\]'
    if(-not $receipt){throw "Missing camera receipt (unknown mode or room?): $root"}
    $capture = if(Test-Path -LiteralPath "$root/camera.png"){'camera.png'}
        elseif(Test-Path -LiteralPath "$root/bench-000.png"){'bench-000.png'}
        else{throw "Place never reached capture: $root"}
    $capture | Set-Content -LiteralPath "$root/capture-file.txt"
    $receipt.Line
    Write-Output $root
} finally {
    if($process -and -not $process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
