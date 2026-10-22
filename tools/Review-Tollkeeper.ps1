param(
    [ValidateSet('Combat','Moving','Duel','Baseline')] [string]$Mode='Moving',
    [switch]$Enraged,
    [switch]$Checkpoint,
    [ValidateRange(8,120)] [int]$Seconds=24,
    [string]$Label='review',
    [string]$Executable='target/release-fast/minecraft_style_voxel_prototype.exe'
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath (Join-Path $repo $Executable)).Path
$root=Join-Path $repo "target/tollkeeper-review/$Label-$Mode"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$frozenExe=Join-Path $root 'review.exe'
Copy-Item -LiteralPath $exe -Destination $frozenExe
$names=@('VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_TOLLKEEPER_MOVE','VOXEL_TOLLKEEPER_CAST','VOXEL_TOLLKEEPER_ENRAGED','VOXEL_TOLLKEEPER_BASELINE','VOXEL_TOLLKEEPER_CHECKPOINT')
$previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_CAPTURE_DIR=$root
    $env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
    $env:VOXEL_TOLLKEEPER_MOVE=if($Mode -in @('Moving','Duel')){'1'}else{$null}
    $env:VOXEL_TOLLKEEPER_CAST=if($Mode -eq 'Duel'){'1'}else{$null}
    $env:VOXEL_TOLLKEEPER_ENRAGED=if($Enraged){'1'}else{$null}
    $env:VOXEL_TOLLKEEPER_BASELINE=if($Mode -eq 'Baseline'){'1'}else{$null}
    $env:VOXEL_TOLLKEEPER_CHECKPOINT=if($Checkpoint){'1'}else{$null}
    $arguments=@('--benchmark-tollkeeper-review','--benchmark-width=1280','--benchmark-height=720',
        '--benchmark-warmup=4',"--benchmark-duration=$Seconds",'--benchmark-capture-hud',
        '--benchmark-weather=clear','--benchmark-review-time=0.5','--benchmark-capture-interval=2')
    [ordered]@{executable=$frozenExe;source_executable=$exe;sha256=(Get-FileHash -LiteralPath $frozenExe).Hash;mode=$Mode;
        starts_enraged=[bool]$Enraged;checkpoint=[bool]$Checkpoint;arguments=$arguments;started_utc=[DateTime]::UtcNow.ToString('o');
        note='Disposable crossing with production AI, terrain, collision and damage. Moving/Duel use normal player movement keys; Duel aims/fires the real wand with normal mana/cooldowns. Baseline omits the actor. No claim of manual play acceptance.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    $process=Start-Process -FilePath $frozenExe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    $process.WaitForExit()
    $log=Join-Path $root 'stdout.log'
    if(-not (Select-String -LiteralPath $log -SimpleMatch '[benchmark] avg_fps=' -Quiet)){throw "Review did not finish: $root"}
    Select-String -LiteralPath $log -Pattern '\[tollkeeper|avg_fps=|gpu_avg_ms' | ForEach-Object {$_.Line}
    Write-Output "Evidence: $root"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
