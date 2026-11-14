param(
    [ValidateRange(0,2)][int]$Tradition=0,
    [ValidateRange(1,3)][int]$Complexity=1,
    [ValidateSet("Legacy","Ordered","Mixed","Organic")][string]$Layout="Legacy",
    [uint32]$LayoutSeed=424242,
    [switch]$LayoutOnly,
    [switch]$Check,
    [switch]$StartSite,
    [switch]$Market,
    [switch]$PreviewTrade,
    [switch]$Capture,
    [switch]$Benchmark,
    [int]$Seconds=45,
    [ValidateRange(0,300)][int]$Warmup=3,
    # x, z, clearance above terrain in metres, yaw, pitch.
    [string]$Camera='22,28,12,-0.65,-0.4',
    [ValidateRange(0,1000000)][int]$PreviewTicks=0,
    [string]$OutputRoot='',
    [string]$Executable='target/debug/minecraft_style_voxel_prototype.exe'
)
$ErrorActionPreference='Stop'
if ($StartSite -and ($Market -or $Layout -ne 'Legacy')) {throw 'Start-site mode uses its own production and project plan.'}
if ($Market -and $Layout -ne 'Legacy') {throw 'Run layout and market fixtures separately.'}
$repo=Split-Path $PSScriptRoot -Parent
if (-not $OutputRoot) {$OutputRoot=Join-Path $repo ('captures/city-quarter-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))}
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot)
New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
$sourceExe=if ([IO.Path]::IsPathRooted($Executable)) {$Executable} else {Join-Path $repo $Executable}
if (-not (Test-Path -LiteralPath $sourceExe)) {throw 'Build the native app first: cargo build --offline -p minecraft_style_voxel_prototype'}
$exe=Join-Path $OutputRoot 'city-quarter.exe'
Copy-Item -LiteralPath $sourceExe -Destination $exe
$names=@('VOXEL_CITY_START_SITE','VOXEL_CITY_LAYOUT','VOXEL_CITY_LAYOUT_SEED','VOXEL_CITY_LAYOUT_ONLY','VOXEL_CITY_PREVIEW_TRADE','VOXEL_CITY_MARKET','VOXEL_CITY_TRADITION','VOXEL_CITY_COMPLEXITY','VOXEL_SAVE_DIR','VOXEL_CAPTURE_DIR')
$previous=@{}
foreach($name in $names) {$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_CITY_START_SITE=if($StartSite){'1'}else{'0'}
    $env:VOXEL_CITY_LAYOUT=$Layout
    $env:VOXEL_CITY_LAYOUT_SEED="$LayoutSeed"
    $env:VOXEL_CITY_LAYOUT_ONLY=if($LayoutOnly){"1"}else{"0"}
    $env:VOXEL_CITY_PREVIEW_TRADE=if($PreviewTrade){'1'}else{'0'}
    $env:VOXEL_CITY_MARKET=if($Market){'1'}else{'0'}
    $env:VOXEL_CITY_TRADITION="$Tradition"
    $env:VOXEL_CITY_COMPLEXITY="$Complexity"
    $env:VOXEL_SAVE_DIR=Join-Path $OutputRoot 'disposable-saves'
    $env:VOXEL_CAPTURE_DIR=$OutputRoot
    $argsList=@('--city-quarter')
    if($PreviewTicks -gt 0) {$argsList+="--city-quarter-preview-ticks=$PreviewTicks"}
    if ($Check) {$argsList=@("--city-quarter-check=$OutputRoot")}
    elseif($Capture -or $Benchmark) {$argsList+=@("--benchmark-warmup=$Warmup","--benchmark-duration=$Seconds",'--benchmark-capture-interval=5','--benchmark-width=1280','--benchmark-height=720','--benchmark-view-chunks=4','--benchmark-review-time=0.4',"--benchmark-terrain-camera=$Camera"); if($Capture){$argsList+='--benchmark-capture-frames'}}
    [ordered]@{started_utc=[DateTime]::UtcNow.ToString('o');executable=$exe;
        sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash;
        start_site=[bool]$StartSite;layout=$Layout;layout_seed=$LayoutSeed;tradition=$Tradition;complexity=$Complexity;market=[bool]$Market;capture_frames=[bool]$Capture;preview_to_first_trade=[bool]$PreviewTrade;arguments=$argsList;fixture=$true} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $OutputRoot 'manifest.json')
    Push-Location -LiteralPath $repo
    try {
        if ($Capture -or $Benchmark) {
            $proc=Start-Process -FilePath $exe -ArgumentList $argsList -WorkingDirectory $repo -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $OutputRoot 'stdout.log') -RedirectStandardError (Join-Path $OutputRoot 'stderr.log')
            $proc.WaitForExit()
            if ($proc.ExitCode -ne 0) {throw "Native capture failed: $($proc.ExitCode)"}
        } else {
            & $exe @argsList
            if($LASTEXITCODE -ne 0){throw "Native app failed: $LASTEXITCODE"}
        }
    } finally {Pop-Location}
} finally {
    foreach($name in $names) {[Environment]::SetEnvironmentVariable($name,$previous[$name])}
}
Write-Output $OutputRoot
