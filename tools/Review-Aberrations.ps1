param(
    [string]$Executable='target/voxel-river-creature-refinement.exe',
    [string]$Label=(Get-Date -Format 'yyyyMMdd-HHmmss'),
    [int]$Seconds=18,
    [int[]]$Kinds=@(26,27,28,29),
    [ValidateSet('hold','camp','obstacle','aberrations')][string]$Scene='hold'
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe=(Resolve-Path -LiteralPath $Executable).Path
$names=@('VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_REVIEW_ENEMIES','VOXEL_ENEMY_REVIEW_KIND',
    'VOXEL_ENEMY_REVIEW_SCENE','VOXEL_ENEMY_REVIEW_SURVIVAL','VOXEL_REVIEW_INPUT_ISOLATED')
$prior=@{};foreach($name in $names){$prior[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    foreach($kind in $Kinds){
        if($kind -lt 26 -or $kind -gt 29){throw "Not an aberration kind: $kind"}
        $root=Join-Path $repo "target/aberration-review/$Label-$Scene-$kind"
        if(Test-Path -LiteralPath $root){throw "Review already exists: $root"}
        New-Item -ItemType Directory -Path $root -Force | Out-Null
        $env:VOXEL_CAPTURE_DIR=$root
        $env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
        $env:VOXEL_REVIEW_ENEMIES='dodge'
        $env:VOXEL_ENEMY_REVIEW_KIND="$kind"
        $env:VOXEL_ENEMY_REVIEW_SCENE=$Scene
        $env:VOXEL_ENEMY_REVIEW_SURVIVAL='1'
        $env:VOXEL_REVIEW_INPUT_ISOLATED='1'
        $argsList=@('--benchmark','--benchmark-world=superflat','--benchmark-width=1280',
            '--benchmark-height=720','--benchmark-weather=clear','--benchmark-sky-time=12',
            '--benchmark-review-time=0.42','--benchmark-view-chunks=4','--benchmark-warmup=5',
            "--benchmark-duration=$Seconds",'--benchmark-physics','--benchmark-capture-frames',
            '--benchmark-capture-interval=2')
        @{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;args=$argsList;
            kind=$kind;scene=$Scene;mode='survival';started_utc=[DateTime]::UtcNow.ToString('o')} |
            ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
        & $exe @argsList *> (Join-Path $root 'runtime.log')
        $LASTEXITCODE | Set-Content -LiteralPath (Join-Path $root 'exit.txt')
        if($LASTEXITCODE -ne 0){throw "Native aberration review exited $LASTEXITCODE"}
        Select-String -LiteralPath (Join-Path $root 'runtime.log') -Pattern 'aberration-cast|review-player' | Select-Object -Last 4
    }
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$prior[$name])}}
