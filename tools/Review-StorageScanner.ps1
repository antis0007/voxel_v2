param(
    [string]$Executable = "$PSScriptRoot/../target/storage-validation/debug/minecraft_style_voxel_prototype.exe",
    [Parameter(Mandatory=$true)][string]$Output,
    [string]$SourceRoot = "$PSScriptRoot/..",
    [switch]$Craft,
    [int]$Width = 1600,
    [int]$Height = 900
)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path -LiteralPath $SourceRoot).Path
$exe=(Resolve-Path -LiteralPath $Executable).Path
if(Test-Path -LiteralPath $Output){throw "Evidence already exists: $Output"}
$root=(New-Item -ItemType Directory -Path $Output).FullName
$arguments=@('--benchmark-world=superflat','--benchmark-seed=424242',
    '--benchmark-generator-version=57','--benchmark-scale=1','--benchmark-weather=clear',
    '--benchmark-sky-time=12','--benchmark-view-chunks=2',
    "--benchmark-width=$Width","--benchmark-height=$Height",'--benchmark-warmup=8',
    '--benchmark-terrain-camera=0,0,1,0,0','--benchmark-duration=15','--benchmark-capture-hud')
$vars=@{VOXEL_CAPTURE_DIR=$root;VOXEL_SAVE_DIR=(Join-Path $root 'disposable-save');
    VOXEL_REVIEW_SCANNER_CRAFT=$(if ($Craft) { '1' } else { $null });
    VOXEL_REVIEW_SCANNER='1';VOXEL_REVIEW_INPUT_ISOLATED='1';VOXEL_REVIEW_SKIP_CLOUDS='1';
    VOXEL_REVIEW_SKIP_HORIZON='1';VOXEL_REVIEW_ENEMIES=$null;VOXEL_REVIEW_DUNGEON=$null;
    VOXEL_REVIEW_WAND=$null;VOXEL_REVIEW_TERRAIN=$null;VOXEL_REVIEW_WATER=$null;
    VOXEL_REVIEW_PANEL=$null;VOXEL_CAPTURE_VIDEO=$null}
$previous=@{};$process=$null
try {
    foreach($name in $vars.Keys){
        $previous[$name]=[Environment]::GetEnvironmentVariable($name)
        if($null -eq $vars[$name]){[Environment]::SetEnvironmentVariable($name,[NullString]::Value)}
        else{[Environment]::SetEnvironmentVariable($name,$vars[$name])}
    }
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;sourceRoot=$repo;arguments=$arguments;environment=$vars;
        scope='Native scanner fixture, real inventory operations and rendering; not multiplayer transport acceptance'} |
        ConvertTo-Json -Depth 4 | Set-Content (Join-Path $root 'manifest.json')
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    $deadline=[DateTime]::UtcNow.AddSeconds(150)
    while(!$process.WaitForExit(2000)){if([DateTime]::UtcNow -gt $deadline){$process.Kill();throw 'Native capture timed out'}}
    if($process.ExitCode -ne 0){throw "Native review exited $($process.ExitCode): $root"}
    if(!(Select-String -LiteralPath (Join-Path $root 'stdout.log') -SimpleMatch '[scanner-native]' -Quiet)){throw 'Missing scanner receipt'}
    if($Craft -and !(Select-String -LiteralPath (Join-Path $root 'stdout.log') -SimpleMatch '[scanner-craft-native]' -Quiet)){throw 'Missing crafting receipt'}
    if(!(Get-ChildItem -LiteralPath $root -Filter '*.png')){throw 'Missing native screenshot'}
    Write-Output $root
} finally {
    if($process -and !$process.HasExited){$process.Kill()}
    foreach($name in $previous.Keys){
        if($null -eq $previous[$name]){[Environment]::SetEnvironmentVariable($name,[NullString]::Value)}
        else{[Environment]::SetEnvironmentVariable($name,$previous[$name])}
    }
}
