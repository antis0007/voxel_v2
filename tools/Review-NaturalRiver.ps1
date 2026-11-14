param(
    [string]$Executable='target/voxel-river-creature-refinement.exe',
    [string]$Label=(Get-Date -Format 'yyyyMMdd-HHmmss'),
    [int]$Seconds=60,
    [int]$GeneratorVersion=0
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe=(Resolve-Path -LiteralPath $Executable).Path
$root=Join-Path $repo "target/natural-river-review/$Label"
if(Test-Path -LiteralPath $root){throw "Review already exists: $root"}
New-Item -ItemType Directory -Path $root -Force | Out-Null
$names=@('VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_RIVER_REVIEW','VOXEL_REVIEW_INPUT_ISOLATED')
$prior=@{};foreach($name in $names){$prior[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_CAPTURE_DIR=$root
    $env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
    $env:VOXEL_RIVER_REVIEW='1'
    $env:VOXEL_REVIEW_INPUT_ISOLATED='1'
    $argsList=@('--benchmark','--benchmark-world=hills',
        '--benchmark-seed=2738958700','--benchmark-width=1600','--benchmark-height=900',
        '--benchmark-review-time=0.42','--benchmark-weather=clear','--benchmark-sky-time=12',
        '--benchmark-terrain-camera=0,0,8,0,-0.12','--benchmark-view-chunks=6',
        '--benchmark-warmup=15',"--benchmark-duration=$Seconds",'--benchmark-physics',
        '--benchmark-capture-frames','--benchmark-capture-interval=10')
    if($GeneratorVersion -gt 0){$argsList+="--benchmark-generator-version=$GeneratorVersion"}
    @{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;args=$argsList;
        started_utc=[DateTime]::UtcNow.ToString('o');terrain_edits=0} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    & $exe @argsList *> (Join-Path $root 'runtime.log')
    $LASTEXITCODE | Set-Content -LiteralPath (Join-Path $root 'exit.txt')
    if($LASTEXITCODE -ne 0){throw "Native river review exited $LASTEXITCODE"}
    Select-String -LiteralPath (Join-Path $root 'runtime.log') -Pattern 'river-review' | Select-Object -Last 5
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$prior[$name])}}
