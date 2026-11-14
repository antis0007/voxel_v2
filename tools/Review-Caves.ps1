param([string]$Executable='target/multiplayer-caves-reviewed.exe', [string]$Label='caves-current')
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$root=Join-Path $repo "target/visual-review/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence already exists: $root"}
New-Item -ItemType Directory -Path $root | Out-Null
$exe=(Resolve-Path -LiteralPath $Executable).Path
$env:VOXEL_RECEIVER_REVIEW='1'
$env:VOXEL_CAPTURE_DIR=$root
$env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
Set-Content -LiteralPath (Join-Path $root 'command.txt') -Value '1 cave-dry'
$arguments=@('--benchmark-world=hills','--benchmark-seed=74','--benchmark-scale=1',
    '--benchmark-weather=clear',
    '--benchmark-width=960','--benchmark-height=540','--benchmark-view-chunks=4',
    '--benchmark-warmup=20','--benchmark-duration=8','--benchmark-capture-frames',
    '--benchmark-capture-hud','--benchmark-capture-interval=2')
$p=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -PassThru `
    -RedirectStandardOutput "$root/native.out" -RedirectStandardError "$root/native.err"
try {
    if(-not $p.WaitForExit(180000)){throw 'Cave review timed out'}
    if($p.ExitCode -ne 0){throw "Cave review exited $($p.ExitCode)"}
    $log=Get-Content "$root/native.out" -Raw
    if($log -notmatch '\[cave-review\].*camera=Some'){throw 'No natural cave camera found'}
    [pscustomobject]@{executable=$exe;sha256=(Get-FileHash $exe -Algorithm SHA256).Hash;
        evidence=$root;exit=$p.ExitCode} | ConvertTo-Json | Set-Content "$root/result.json"
} finally {
    if(-not $p.HasExited){Stop-Process -Id $p.Id -Force}
}
