param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [string]$RegionLocator,
    [string]$LocationsFile,
    [Parameter(Mandatory=$true)][string]$OutputRoot,
    [uint32]$Seed=74,
    [uint32[]]$Generators=@(138,141),
    [ValidateSet('1m','0.5m','0.25m')][string[]]$Scales=@('1m','0.5m','0.25m'),
    [string[]]$Views=@('high','medium','corridor','valley','transition','ground'),
    [double]$ReviewTime=0.5,
    [int]$Warmup=30
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath $Executable).Path
if(-not $RegionLocator -and -not $LocationsFile){throw 'Supply RegionLocator or its recorded LocationsFile'}
if(Test-Path -LiteralPath $OutputRoot){throw "Preserve existing evidence: $OutputRoot"}
$root=(New-Item -ItemType Directory -Path $OutputRoot).FullName
# Freeze the binary so another build cannot replace it between matched views.
$frozen=Join-Path $root 'region-review.exe'
Copy-Item -LiteralPath $exe -Destination $frozen
Get-ChildItem -LiteralPath (Split-Path $exe) -Filter '*.dll' -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $root
}
$sha=(Get-FileHash -LiteralPath $frozen).Hash
$locations=if($LocationsFile){@(Get-Content -LiteralPath $LocationsFile)}else{
    $locator=(Resolve-Path -LiteralPath $RegionLocator).Path
    $lines=@(& $locator $Seed)
    if($LASTEXITCODE -ne 0){throw 'Region locator failed'}
    $lines
}
$locations | Set-Content -LiteralPath (Join-Path $root 'region.txt')
if(-not($locations -match "^seed=$Seed reach=")){throw "Locations do not describe seed $Seed"}
$poses=@{}
foreach($line in $locations){if($line -match '^(\w+) camera=(.+)$'){$poses[$Matches[1]]=$Matches[2]}}
foreach($view in $Views){if(-not $poses.ContainsKey($view)){throw "Missing camera: $view"}}
$vars=@{VOXEL_REVIEW_WAND='2';VOXEL_REVIEW_DUNGEON=$null;VOXEL_REVIEW_WATER=$null;
    VOXEL_REVIEW_TERRAIN=$null;VOXEL_REVIEW_SKIP_HORIZON=$null;VOXEL_REVIEW_SKIP_CLOUDS=$null;
    VOXEL_TERRAIN_ROUTE=$null;VOXEL_CAPTURE_DIR=$null;VOXEL_SAVE_DIR=$null}
$previous=@{}
$missingCaptures=[Collections.Generic.List[string]]::new()
foreach($key in $vars.Keys){$previous[$key]=[Environment]::GetEnvironmentVariable($key)}
try {
    foreach($key in $vars.Keys){[Environment]::SetEnvironmentVariable($key,$vars[$key])}
    foreach($version in $Generators){foreach($scale in $Scales){foreach($view in $Views){
        $scaleArg=@{'1m'='1';'0.5m'='0.5';'0.25m'='0.25'}[$scale]
        $dir=(New-Item -ItemType Directory -Path (Join-Path $root "v$version-$scale-$view")).FullName
        $env:VOXEL_CAPTURE_DIR=$dir
        $env:VOXEL_SAVE_DIR=Join-Path $dir 'disposable-world'
        $arguments=@('--benchmark-world=planet',"--benchmark-seed=$Seed","--benchmark-generator-version=$version",
            "--benchmark-scale=$scaleArg",'--benchmark-weather=clear',"--benchmark-review-time=$ReviewTime",'--benchmark-sky-time=12',
            '--benchmark-view-chunks=8','--benchmark-cloud-divisor=3','--benchmark-width=1280','--benchmark-height=720',
            "--benchmark-terrain-camera=$($poses[$view])","--benchmark-warmup=$Warmup",'--benchmark-duration=3',
            '--benchmark-capture-frames','--benchmark-capture-interval=1')
        [ordered]@{executable=$frozen;sha256=$sha;seed=$Seed;generator=$version;voxelScale=$scale;view=$view;
            camera=$poses[$view];arguments=$arguments;environment=@{VOXEL_CAPTURE_DIR=$dir;VOXEL_SAVE_DIR=$env:VOXEL_SAVE_DIR};
            scope='Native camera-only visual review. Capture timing includes readback; no traversal or performance acceptance.'} |
            ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $dir 'manifest.json')
        $p=Start-Process -FilePath $frozen -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput (Join-Path $dir 'stdout.log') -RedirectStandardError (Join-Path $dir 'stderr.log')
        $null=$p.Handle
        $deadline=[DateTime]::UtcNow.AddSeconds(180)
        $peakWorkingSetBytes=0L
        $peakPrivateBytes=0L
        while(-not $p.WaitForExit(500)){
            try {
                $p.Refresh()
                $peakWorkingSetBytes=[Math]::Max($peakWorkingSetBytes,$p.WorkingSet64)
                $peakPrivateBytes=[Math]::Max($peakPrivateBytes,$p.PrivateMemorySize64)
            } catch [InvalidOperationException] {
                if(-not $p.HasExited){throw}
            }
            if([DateTime]::UtcNow -gt $deadline){$p.Kill();throw "Capture timed out: v$version-$view"}
        }
        @{sampleIntervalMs=500;peakSampledWorkingSetBytes=$peakWorkingSetBytes;
            peakSampledPrivateBytes=$peakPrivateBytes;exitCode=$p.ExitCode} |
            ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dir 'process.json')
        if($p.ExitCode -ne 0){throw "Capture failed: v$version-$view ($($p.ExitCode))"}
        $capture=Join-Path $dir 'camera.png'
        if(-not(Test-Path -LiteralPath $capture)){
            Write-Warning "No settled camera capture: v$version-$view; inspect streaming logs."
            $frame=Get-ChildItem -LiteralPath $dir -Filter 'bench-*.png' -File | Sort-Object Name | Select-Object -Last 1
            if(-not $frame){
                $missingCaptures.Add("v$version-$scale-$view")
                continue
            }
            $capture=$frame.FullName
        }
        Write-Output $capture
    }}}
    if($missingCaptures.Count){throw "No renderer image for: $($missingCaptures -join ', ')"}
} finally {
    if($p -and -not $p.HasExited){$p.Kill();$p.WaitForExit()}
    foreach($key in $previous.Keys){[Environment]::SetEnvironmentVariable($key,$previous[$key])}
}
