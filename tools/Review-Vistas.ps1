param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$OutputRoot,
    [string[]]$Views=@('arch','buttes','shelf'),
    [uint32]$Seed=424242,
    [uint32]$Generator=99,
    [double]$TimeOfDay=0.5,
    [string]$Scale='1',
    [int]$ViewChunks=12,
    [int]$Warmup=100,
    [switch]$SkipHorizon
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
foreach($view in $Views){
    $root=Join-Path $OutputRoot $view
    if(Test-Path -LiteralPath $root){throw "Capture already exists: $root"}
    New-Item -ItemType Directory -Path $root | Out-Null
    $vars=@{VOXEL_REVIEW_DUNGEON="vista:$view";VOXEL_SAVE_DIR=(Join-Path $root 'disposable-world');VOXEL_CAPTURE_DIR=$root;VOXEL_REVIEW_SKIP_HORIZON=$(if($SkipHorizon){'1'}else{$null});VOXEL_REVIEW_SKIP_CLOUDS=$null}
    $previous=@{}
    try {
        foreach($key in $vars.Keys){$previous[$key]=[Environment]::GetEnvironmentVariable($key);[Environment]::SetEnvironmentVariable($key,$vars[$key])}
        $arguments=@('--benchmark-world=hills',"--benchmark-seed=$Seed","--benchmark-generator-version=$Generator","--benchmark-scale=$Scale",'--benchmark-weather=clear',"--benchmark-review-time=$($TimeOfDay.ToString([Globalization.CultureInfo]::InvariantCulture))","--benchmark-view-chunks=$ViewChunks",'--benchmark-width=1600','--benchmark-height=900','--benchmark-terrain-camera=0,0,1,0,0',"--benchmark-warmup=$Warmup",'--benchmark-duration=3','--benchmark-capture-frames')
        [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;environment=$vars;scope='Native generated terrain and architecture; camera-only review'}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath "$root/manifest.json"
        $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
        $deadline=[DateTime]::UtcNow.AddSeconds(240)
        while(-not $process.WaitForExit(500)){if([DateTime]::UtcNow -gt $deadline){$process.Kill();throw "Capture timed out: $view"}}
        if($process.ExitCode -ne 0){throw "Capture failed: $view ($($process.ExitCode))"}
        $capture=if(Test-Path -LiteralPath "$root/camera.png"){'camera.png'}elseif(Test-Path -LiteralPath "$root/bench-000.png"){'bench-000.png'}else{throw "Capture missing: $view"}
        if($capture -ne 'camera.png'){Write-Warning "Streaming did not produce a settled camera capture: $view. Inspect stdout.log before using the fallback image."}
        $receipt=Select-String -LiteralPath "$root/stdout.log" -Pattern '^\[dungeon-review\]'
        if(-not $receipt){throw "No generated vista camera found: $view"}
        $receipt.Line
        Write-Output "$root/$capture"
    } finally {
        if($process -and -not $process.HasExited){$process.Kill();$process.WaitForExit()}
        foreach($key in $previous.Keys){[Environment]::SetEnvironmentVariable($key,$previous[$key])}
    }
}
