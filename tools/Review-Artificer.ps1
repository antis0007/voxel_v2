param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [ValidateSet('outdoors','studio','hall','night','motion','workshop')][string]$Mode='outdoors',
    [ValidateRange(0,5)][int]$Shaft=0,
    [ValidateRange(0,7)][int]$Head=0,
    [ValidateRange(0,5)][int]$Grip=0,
    [string]$Pose='0',
    [ValidateSet('','stone','grass','planks','crystal','torch','food','sword','gem')][string]$Held='',
    [switch]$Avatars,
    [ValidateSet('','bone_needle','runic_key','cinder_rod','artificer_tally')][string]$Item='',
    [ValidateRange(-1,95)][int]$Spell=-1,
    [int]$Seconds=8,
    [int]$Warmup=10,
    [int]$Width=1280,
    [int]$Height=720,
    [double]$CaptureInterval=0.20,
    [switch]$NoCapture
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "target/artificer-20261025/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
$save=Join-Path $root 'generated-world'
New-Item -ItemType Directory -Path $save | Out-Null
"shaft=$Shaft`nhead=$Head`ntrail=1`ngrip=$Grip`nglow=0`nrunes=1`n" |
    Set-Content -LiteralPath (Join-Path $save 'wand-style.txt')
$day=if($Mode -eq 'night'){'0.05'}else{'0.5'}
$seed=if($Mode -eq 'workshop'){'424242'}else{'2738958700'}
$generator=if($Mode -eq 'workshop'){'57'}else{'48'}
$arguments=@('--benchmark-world=hills',"--benchmark-seed=$seed",
    "--benchmark-generator-version=$generator",'--benchmark-weather=clear',
    "--benchmark-review-time=$day",'--benchmark-sky-time=12',
    '--benchmark-view-chunks=6',"--benchmark-width=$Width","--benchmark-height=$Height",
    '--benchmark-terrain-camera=44,35,1,-1.0,-0.12',
    "--benchmark-warmup=$Warmup","--benchmark-duration=$Seconds")
if(-not $NoCapture){$arguments+='--benchmark-capture-frames'}
if($Mode -eq 'motion'){$arguments+="--benchmark-capture-interval=$CaptureInterval"}
$vars=@{
    VOXEL_REVIEW_HELD=if($Held){$Held}else{$null};
    VOXEL_REVIEW_AVATARS=if($Avatars){'1'}else{$null};
    VOXEL_REVIEW_WAND_GIRTH=$null; VOXEL_REVIEW_WAND_PIXEL_MM=$null;
    VOXEL_SAVE_DIR=$save; VOXEL_CAPTURE_DIR=$root; VOXEL_REVIEW_WAND='2';
    VOXEL_REVIEW_ITEM=if($Item){$Item}else{$null};
    VOXEL_REVIEW_WAND_SPELL=if($Spell -ge 0){[string]$Spell}else{$null};
    VOXEL_VIEWMODEL_TIME=if($Mode -eq 'motion'){$null}else{'0'};
    VOXEL_VIEWMODEL_POSE=if($Mode -eq 'motion'){'cycle'}else{$Pose};
    VOXEL_VIEWMODEL_STUDIO=if($Mode -eq 'studio'){'1'}else{$null};
    VOXEL_REVIEW_DUNGEON=if($Mode -eq 'hall'){'entry'}elseif($Mode -eq 'workshop'){'place:workshop-inside'}else{$null};
    VOXEL_REVIEW_SKIP_CLOUDS='1'; VOXEL_REVIEW_INPUT_ISOLATED='1';
    VOXEL_TERRAIN_ROUTE=if($Mode -eq 'motion'){'1'}else{$null};
    VOXEL_TERRAIN_EDITS=$null; VOXEL_TERRAIN_LIGHT=$null;
    VOXEL_REVIEW_TERRAIN=$null; VOXEL_REVIEW_ENEMIES=$null;
    VOXEL_REVIEW_WATER=$null; VOXEL_CAPTURE_VIDEO=$null;
}
$previous=@{}
$process=$null
try {
    foreach($name in $vars.Keys){
        $previous[$name]=[Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name,$vars[$name])
    }
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;
        arguments=$arguments;environment=$vars;
        scope='Native visual review; capture readbacks and concurrent work exclude performance acceptance'} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath "$root/manifest.json"
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput "$root/stdout.log" `
        -RedirectStandardError "$root/stderr.log"
    $null=$process.Handle
    $process.Id | Set-Content -LiteralPath "$root/pid.txt"
    $deadline=[DateTime]::UtcNow.AddSeconds(180)
    while(-not $process.WaitForExit(500)){
        if([DateTime]::UtcNow -gt $deadline){throw 'Native review exceeded 180 seconds'}
    }
    [ordered]@{exit_code=$process.ExitCode;pid=$process.Id} |
        ConvertTo-Json | Set-Content -LiteralPath "$root/process.json"
    if($process.ExitCode -ne 0){throw "Native review exited $($process.ExitCode)"}
    if(-not $NoCapture -and -not(Test-Path -LiteralPath "$root/camera.png")){
        throw 'Missing resident scene capture'
    }
    if(Select-String -LiteralPath "$root/stderr.log" -Pattern '\[wand-asset\].*:|\[wand-kit\] assets/wands.ron:' -Quiet){
        throw 'Authored asset parse failed'
    }
    Write-Output $root
} finally {
    if($process -and -not $process.HasExited){$process.Kill();$process.WaitForExit()}
    foreach($name in $previous.Keys){[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
