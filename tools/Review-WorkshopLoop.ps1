param(
    [string]$Executable='target/release-fast/minecraft_style_voxel_prototype.exe',
    [string]$Label='workshop-loop',
    [int]$Seconds=200,
    [switch]$LightRendering
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$source=(Resolve-Path -LiteralPath (Join-Path $repo $Executable)).Path
$root=Join-Path $repo "target/workshop-loop/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$exe=Join-Path $root 'candidate.exe'
Copy-Item -LiteralPath $source -Destination $exe
$arguments=@('--benchmark-world=hills','--benchmark-motion=static','--benchmark-seed=2738958700',
    '--benchmark-width=1280','--benchmark-height=720','--benchmark-view-chunks=4',
    '--benchmark-warmup=3',"--benchmark-duration=$Seconds",'--benchmark-capture-hud',
    '--benchmark-capture-interval=10','--benchmark-weather=clear','--benchmark-review-time=0.5','--benchmark-physics')
$names=@('VOXEL_WORKSHOP_REVIEW','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_REVIEW_SKIP_CLOUDS','VOXEL_REVIEW_SKIP_HORIZON')
$prior=@{}
foreach($name in $names){$prior[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_WORKSHOP_REVIEW='1'
    $env:VOXEL_CAPTURE_DIR=$root
    $env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
    $env:VOXEL_REVIEW_SKIP_CLOUDS=if($LightRendering){'1'}else{$null}
    $env:VOXEL_REVIEW_SKIP_HORIZON=if($LightRendering){'1'}else{$null}
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;arguments=$arguments;light_rendering=[bool]$LightRendering;
        started_utc=[DateTime]::UtcNow.ToString('o');note='Generated starter workshop/mine. Production movement, collision, mining inputs, casting and checkpoint service. Station actions use production handlers. No camera route, fixture terrain, granted loot or forced victories.'} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    $p=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    $null=$p.Handle
    $p.WaitForExit()
    if($p.ExitCode -ne 0){throw "Native review exited $($p.ExitCode): $root"}
    if(Select-String -LiteralPath (Join-Path $root 'stderr.log') -Pattern '\[renderer-fatal\]|\[device-lost\]|panicked' -Quiet){
        throw "Native review encountered a fatal runtime error: $root"
    }
    if(-not(Select-String -LiteralPath (Join-Path $root 'stdout.log') -SimpleMatch '[workshop-loop] PASS' -Quiet)){
        throw "Route did not pass: $root"
    }
    if(-not(Select-String -LiteralPath (Join-Path $root 'stdout.log') -SimpleMatch '[workshop-loop] reloaded world rendered' -Quiet)){
        throw "Checkpoint matched, but the reloaded world was not rendered: $root"
    }
    Select-String -LiteralPath (Join-Path $root 'stdout.log') -Pattern '\[workshop-loop\]' | ForEach-Object {$_.Line}
} finally { foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$prior[$name])} }
