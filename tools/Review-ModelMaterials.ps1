param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$Label,
    [string]$DebugView = '',
    [int]$Seconds = 6
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$exe = (Resolve-Path -LiteralPath $Executable).Path
$root = Join-Path $repo "target/model-material-review/$Label"
if (Test-Path -LiteralPath $root) { throw "Evidence already exists: $root" }
New-Item -ItemType Directory -Path $root | Out-Null
$arguments = @('--benchmark-world=superflat','--benchmark-seed=2738958700',
    '--benchmark-generator-version=37','--benchmark-scale=1','--benchmark-weather=clear',
    '--benchmark-review-time=0.5','--benchmark-sky-time=12','--benchmark-view-chunks=4',
    '--benchmark-width=1280','--benchmark-height=720','--benchmark-warmup=6',
    '--benchmark-terrain-camera=0,0,5,0,0',
    "--benchmark-duration=$Seconds",'--benchmark-capture-frames',
    '--benchmark-capture-interval=2','--benchmark-capture-hud')
if ($DebugView) { $arguments += "--benchmark-debug=$DebugView" }
$vars = @{
    VOXEL_CAPTURE_DIR=$root; VOXEL_SAVE_DIR=(Join-Path $root 'disposable-save');
    VOXEL_REVIEW_ENEMIES='materials'; VOXEL_ENEMY_REVIEW_SCENE='pack';
    VOXEL_REVIEW_INPUT_ISOLATED='1'; VOXEL_REVIEW_SKIP_CLOUDS='1';
    VOXEL_REVIEW_WAND=$null; VOXEL_REVIEW_TERRAIN=$null;
    VOXEL_REVIEW_DUNGEON=$null; VOXEL_REVIEW_WATER=$null
}
$previous = @{}
try {
    foreach ($name in $vars.Keys) {
        $previous[$name] = [Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name, $vars[$name])
    }
    [ordered]@{executable=$exe; sha256=(Get-FileHash -LiteralPath $exe).Hash;
        arguments=$arguments; environment=$vars;
        scope='Production model/shader diagnostic arena; held tool pose forced, not a gameplay walkthrough or original screenshot location.'} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath "$root/manifest.json"
    $process = Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput "$root/stdout.log" -RedirectStandardError "$root/stderr.log"
    $null = $process.Handle
    $process.Id | Set-Content -LiteralPath "$root/pid.txt"
    if (-not $process.WaitForExit(120000)) {
        $process.Kill(); $process.WaitForExit()
        throw "Material review exceeded 120 seconds: $root"
    }
    [ordered]@{pid=$process.Id; exit_code=$process.ExitCode} |
        ConvertTo-Json | Set-Content -LiteralPath "$root/process.json"
    if ($process.ExitCode -ne 0) { throw "Renderer exited $($process.ExitCode): $root" }
    if (-not (Select-String -LiteralPath "$root/stdout.log" -SimpleMatch '[benchmark] avg_fps=' -Quiet) -or
        -not (Test-Path -LiteralPath "$root/bench-000.png")) {
        throw "Renderer did not finish and capture the scene: $root"
    }
    Select-String -LiteralPath "$root/stdout.log" -Pattern '\[benchmark\]|\[enemy-review\]' |
        ForEach-Object { $_.Line }
    Write-Output "Evidence: $root"
} finally {
    foreach ($name in $vars.Keys) { [Environment]::SetEnvironmentVariable($name,$previous[$name]) }
}
