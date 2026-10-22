param(
    [ValidateSet('duel','dodge','progression','manual')] [string]$Mode='duel',
    [ValidateRange(0,13)] [int]$Kind=6,
    [ValidateSet('moving','camp','ambush','cache','obstacle','pack')] [string]$Scene='moving',
    [ValidateRange(12,180)] [int]$Seconds=30,
    [string]$Label='encounter',
    [switch]$Enraged
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=Join-Path $repo 'target/release-fast/minecraft_style_voxel_prototype.exe'
$root=Join-Path $repo "target/gameplay-review/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$names=@('VOXEL_REVIEW_ENEMIES','VOXEL_ENEMY_REVIEW_KIND','VOXEL_ENEMY_REVIEW_SCENE','VOXEL_ENEMY_REVIEW_ENRAGED','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR')
$previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_REVIEW_ENEMIES=$Mode
    $env:VOXEL_ENEMY_REVIEW_KIND="$Kind"
    $env:VOXEL_ENEMY_REVIEW_SCENE=$Scene
    $env:VOXEL_ENEMY_REVIEW_ENRAGED=if($Enraged){'1'}else{$null}
    $env:VOXEL_CAPTURE_DIR=$root
    $env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
    $arguments=@('--benchmark-world=superflat','--benchmark-motion=static','--benchmark-width=1280','--benchmark-height=720',
        '--benchmark-view-chunks=4','--benchmark-warmup=3',"--benchmark-duration=$Seconds",'--benchmark-capture-hud',
        '--benchmark-capture-interval=4','--benchmark-weather=clear','--benchmark-review-time=0.5')
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;mode=$Mode;kind=$Kind;scene=$Scene;enraged=[bool]$Enraged;
        arguments=$arguments;started_utc=[DateTime]::UtcNow.ToString('o');
        note='Disposable native arena. Production AI, player physics, collision, spell deck, mana, damage, defeat and reward code. Automated modes script ordinary player inputs; manual mode permits mouse capture. No health/mana refills or forced victories.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    Push-Location $repo
    try { & $exe @arguments 1> (Join-Path $root 'stdout.log') 2> (Join-Path $root 'stderr.log') }
    finally { Pop-Location }
    $gameExit=$LASTEXITCODE
    if($gameExit -ne 0){throw "Game exited $gameExit; see $root"}
    $log=Join-Path $root 'stdout.log'
    if(-not(Select-String -LiteralPath $log -SimpleMatch '[benchmark] avg_fps=' -Quiet)){throw "Review did not complete: $root"}
    Select-String -LiteralPath $log -Pattern '\[progression-live\]|\[progression-disk\]|\[benchmark\] avg_fps=' | ForEach-Object {$_.Line}
    Write-Output "Evidence: $root"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
