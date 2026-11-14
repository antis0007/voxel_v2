param(
    [string]$Executable='target/cave-life-reviewed.exe',
    [string]$Label="cenote-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [ValidateSet('cenote','cenote-night','cenote-entry')][string]$View='cenote'
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$root=Join-Path $repo "target/visual-review/$Label"
if(Test-Path -LiteralPath $root){throw 'Use a new evidence directory'}
New-Item -ItemType Directory -Path $root | Out-Null
$exe=(Resolve-Path -LiteralPath (Join-Path $repo $Executable)).Path
$info=[Diagnostics.ProcessStartInfo]::new($exe)
$info.WorkingDirectory=$repo
$info.UseShellExecute=$false
$info.CreateNoWindow=$true
$info.RedirectStandardOutput=$true
$info.RedirectStandardError=$true
$info.Environment['VOXEL_RECEIVER_REVIEW']='1'
$info.Environment['VOXEL_CAPTURE_DIR']=$root
$info.Environment['VOXEL_SAVE_DIR']=Join-Path $root 'disposable-saves'
Set-Content -LiteralPath "$root/command.txt" -Value "1 $View"
foreach($arg in @('--benchmark-world=hills','--benchmark-seed=74','--benchmark-scale=1',
    '--benchmark-weather=clear','--benchmark-width=1280','--benchmark-height=720',
    '--benchmark-view-chunks=4','--benchmark-warmup=30','--benchmark-duration=10',
    '--benchmark-capture-frames','--benchmark-capture-hud','--benchmark-capture-interval=3')){
    $info.ArgumentList.Add($arg)
}
$p=[Diagnostics.Process]::Start($info)
$out=[IO.FileStream]::new("$root/native.out",[IO.FileMode]::Create,[IO.FileAccess]::Write,[IO.FileShare]::ReadWrite,1,$true)
$err=[IO.FileStream]::new("$root/native.err",[IO.FileMode]::Create,[IO.FileAccess]::Write,[IO.FileShare]::ReadWrite,1,$true)
$copyOut=$p.StandardOutput.BaseStream.CopyToAsync($out)
$copyErr=$p.StandardError.BaseStream.CopyToAsync($err)
try {
    if(-not $p.WaitForExit(180000)){throw 'Cenote review timed out'}
    $copyOut.GetAwaiter().GetResult();$copyErr.GetAwaiter().GetResult()
    if($p.ExitCode -ne 0){throw "Game exited $($p.ExitCode)"}
    if((Get-Content "$root/native.out" -Raw) -notmatch '\[cenote-review\].*garden_m='){throw 'No natural cenote found'}
    [pscustomobject]@{executable=$exe;sha256=(Get-FileHash $exe).Hash;view=$View;exit=$p.ExitCode;evidence=$root} |
        ConvertTo-Json | Set-Content "$root/result.json"
} finally {
    if(-not $p.HasExited){$p.Kill();$p.WaitForExit()}
    $copyOut.GetAwaiter().GetResult();$copyErr.GetAwaiter().GetResult()
    $out.Dispose();$err.Dispose();$p.Dispose()
}
