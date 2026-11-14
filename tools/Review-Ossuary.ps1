param(
    [ValidateSet('Manual','Combat','Engineering','Cutting','Baseline','Damaged','Disabled')][string]$Mode='Manual',
    [ValidateRange(8,180)][int]$Seconds=32,
    [switch]$Checkpoint,
    [string]$Label='review',
    [string]$Executable='target/release-fast/minecraft_style_voxel_prototype.exe'
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=(Resolve-Path -LiteralPath (Join-Path $repo $Executable)).Path
$root=Join-Path $repo "target/ossuary-review/$Label-$Mode"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$frozen=Join-Path $root 'review.exe'
Copy-Item -LiteralPath $exe -Destination $frozen
$names=@('VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_OSSUARY_STATE','VOXEL_OSSUARY_CHECKPOINT')
$previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_OSSUARY_CHECKPOINT=if($Checkpoint){'1'}else{$null}
    $env:VOXEL_CAPTURE_DIR=$root
    $env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
    $env:VOXEL_OSSUARY_STATE=if($Mode -eq 'Manual'){''}else{$Mode.ToLowerInvariant()}
    $arguments=@('--benchmark-ossuary-review','--benchmark-width=1280','--benchmark-height=720',
        '--benchmark-warmup=4',"--benchmark-duration=$Seconds",'--benchmark-capture-hud',
        '--benchmark-weather=clear','--benchmark-review-time=0.5','--benchmark-capture-interval=4')
    [ordered]@{executable=$frozen;sha256=(Get-FileHash -LiteralPath $frozen).Hash;mode=$Mode;
        arguments=$arguments;automated=($Mode -in @('Combat','Engineering'));
        note='Disposable world. Automated modes use normal movement/casting and the focused relic interaction. No manual feel acceptance.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    $p=Start-Process -FilePath $frozen -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    if(-not $p.WaitForExit(($Seconds+45)*1000)) {
        throw "Review did not exit within its bounded capture interval: $root"
    }
    $p.ExitCode | Set-Content -LiteralPath (Join-Path $root 'exit-code.txt')
    if($p.ExitCode -ne 0){throw "Review exited $($p.ExitCode): $root"}
    if(-not (Select-String -LiteralPath (Join-Path $root 'stdout.log') -SimpleMatch '[benchmark] avg_fps=' -Quiet)){throw "Missing timing receipt: $root"}
    Select-String -LiteralPath (Join-Path $root 'stdout.log') -Pattern 'complete=True|complete=true|avg_fps=|gpu_avg_ms' | Select-Object -Last 4 | ForEach-Object {$_.Line}
    Write-Output "Evidence: $root"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
