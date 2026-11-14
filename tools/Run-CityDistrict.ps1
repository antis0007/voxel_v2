param(
    [ValidateSet('Ordered','Mixed','Organic')][string]$Layout='Ordered',
    [uint32]$Seed=7,
    [ValidateRange(0,2)][int]$Tradition=0,
    [switch]$Market,
    [switch]$Check,
    [switch]$Matrix,
    [switch]$Capture,
    [ValidateRange(0,1000000)][int]$PreviewTicks=300000,
    [string]$Executable='target/debug/minecraft_style_voxel_prototype.exe',
    [string]$OutputRoot='',
    [string]$Camera='50,52,48,-0.72,-0.62'
)
$ErrorActionPreference='Stop'
if($Market -and $Tradition -ne 0){throw 'Live timber barter requires Tradition 0.'}
$repo=Split-Path $PSScriptRoot -Parent
$exe=if([IO.Path]::IsPathRooted($Executable)){$Executable}else{Join-Path $repo $Executable}
if(-not(Test-Path -LiteralPath $exe)){throw 'Build the district-enabled executable first.'}
if(-not $OutputRoot){$OutputRoot=Join-Path $repo ('captures/district-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff'))}
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot)
if(Test-Path -LiteralPath $OutputRoot){throw 'Choose a new disposable output directory.'}
New-Item -ItemType Directory -Path $OutputRoot | Out-Null
$settings=@{
    VOXEL_CITY_START_SITE='0';VOXEL_CITY_COLONY='0';VOXEL_CITY_MULTISTOREY='0';VOXEL_CITY_DISTRICTS='1';
    VOXEL_CITY_LAYOUT=$Layout;VOXEL_CITY_LAYOUT_SEED="$Seed";VOXEL_CITY_TRADITION="$Tradition";
    VOXEL_CITY_COMPLEXITY='1';VOXEL_CITY_MARKET=$(if($Market){'1'}else{'0'});
    VOXEL_CITY_LAYOUT_ONLY=$(if($Matrix){'1'}else{'0'});VOXEL_CITY_PREVIEW_TRADE='0';
    VOXEL_SAVE_DIR=(Join-Path $OutputRoot 'disposable-saves');VOXEL_CAPTURE_DIR=$OutputRoot
}
$before=@{}
foreach($key in $settings.Keys){$before[$key]=[Environment]::GetEnvironmentVariable($key)}
try {
    foreach($key in $settings.Keys){[Environment]::SetEnvironmentVariable($key,$settings[$key])}
    $arguments=@('--city-quarter',"--city-quarter-preview-ticks=$PreviewTicks")
    if($Check -or $Matrix){$arguments=@("--city-quarter-check=$OutputRoot")}
    elseif($Capture){$arguments+=@('--benchmark-warmup=1','--benchmark-duration=6','--benchmark-capture-interval=5',
        '--benchmark-width=1280','--benchmark-height=720','--benchmark-view-chunks=4','--benchmark-review-time=0.4',
        '--benchmark-weather=clear','--benchmark-capture-frames',"--benchmark-terrain-camera=$Camera")}
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;
        layout=$Layout;seed=$Seed;tradition=$Tradition;market=[bool]$Market;fixture=$true;settings=$settings} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $OutputRoot 'manifest.json')
    if($Capture -or $Check -or $Matrix){
        $process=Start-Process -FilePath $exe -ArgumentList ($arguments | ForEach-Object { '"' + $_ + '"' }) -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput (Join-Path $OutputRoot 'stdout.log') -RedirectStandardError (Join-Path $OutputRoot 'stderr.log')
        $null=$process.Handle;$process.WaitForExit();$process.Refresh()
        if($process.ExitCode -ne 0){throw "District check failed: $($process.ExitCode). See $OutputRoot."}
        if($Capture -and -not(Test-Path -LiteralPath (Join-Path $OutputRoot 'bench-000.png'))){throw 'Capture missing.'}
    } else {
        Push-Location -LiteralPath $repo
        try{& $exe @arguments;if($LASTEXITCODE -ne 0){throw "District exited: $LASTEXITCODE"}}finally{Pop-Location}
    }
}finally{foreach($key in $before.Keys){[Environment]::SetEnvironmentVariable($key,$before[$key])}}
Write-Output $OutputRoot
