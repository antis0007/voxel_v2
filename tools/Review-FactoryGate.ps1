param(
    [string]$Label = 'candidate',
    [string]$Executable = 'target/release/minecraft_style_voxel_prototype.exe',
    [ValidateSet('production','battery','matrix','transformer','assembler','cable')]
    [string]$Panel = 'production',
    [double]$UiScale = 1.0,
    [int]$Seconds = 16
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe = (Resolve-Path -LiteralPath $Executable).Path
$root = Join-Path $repo "target/integrated-gate-a/$Label/$Panel-$UiScale"
if (Test-Path -LiteralPath $root) { throw 'Choose a new label to preserve existing evidence.' }
New-Item -ItemType Directory -Path $root | Out-Null
$argsList = @('--benchmark-torch-review', '--benchmark-world=hills',
    '--benchmark-seed=2738958700', '--benchmark-weather=partly',
    '--benchmark-review-time=0.5', '--benchmark-sky-time=12',
    '--benchmark-cloud-divisor=3', '--benchmark-view-chunks=4',
    '--benchmark-width=1280', '--benchmark-height=720', '--benchmark-warmup=4',
    "--benchmark-duration=$Seconds", '--benchmark-capture-hud')
$names = @('VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR','VOXEL_TORCH_REVIEW_MODE',
    'VOXEL_REVIEW_PANEL','VOXEL_REVIEW_FOCUS','VOXEL_REVIEW_UI_SCALE')
$prior = @{}
foreach ($name in $names) { $prior[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:VOXEL_CAPTURE_DIR = $root
    $env:VOXEL_SAVE_DIR = Join-Path $root 'isolated-saves'
    $env:VOXEL_TORCH_REVIEW_MODE = 'factory'
    $env:VOXEL_REVIEW_PANEL = if ($Panel -eq 'production') { '7' } else { '3' }
    $env:VOXEL_REVIEW_FOCUS = if ($Panel -eq 'production') { $null } else { $Panel }
    $env:VOXEL_REVIEW_UI_SCALE = $UiScale.ToString([Globalization.CultureInfo]::InvariantCulture)
    [ordered]@{executable=$exe;sha256=(Get-FileHash $exe).Hash;arguments=$argsList;
        panel=$Panel;ui_scale=$UiScale;time='noon';weather='partly';seed=2738958700;
        note='Native factory fixture, isolated storage. Does not prove ordinary-player progression.'} |
        ConvertTo-Json -Depth 4 | Set-Content (Join-Path $root 'manifest.json')
    $p = Start-Process -FilePath $exe -ArgumentList $argsList -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    $null = $p.Handle
    $p.WaitForExit()
    if ($p.ExitCode -ne 0) { throw "Renderer exited $($p.ExitCode); inspect $root" }
    if (-not (Select-String -LiteralPath (Join-Path $root 'stdout.log') -Pattern '[benchmark] avg_fps=' -SimpleMatch -Quiet)) {
        throw "Renderer did not finish the benchmark: $root"
    }
    if (-not (Get-ChildItem $root -Filter '*.png')) { throw "No captures written: $root" }
    $captures = Get-ChildItem $root -Filter '*.png' | ForEach-Object {
        $stream = [IO.File]::OpenRead($_.FullName)
        try { $header = New-Object byte[] 24; $null = $stream.Read($header,0,24) } finally { $stream.Dispose() }
        [pscustomobject]@{file=$_.Name;width=([int]$header[16]*16777216+[int]$header[17]*65536+[int]$header[18]*256+$header[19]);
            height=([int]$header[20]*16777216+[int]$header[21]*65536+[int]$header[22]*256+$header[23])}
    }
    $captures | ConvertTo-Json | Set-Content (Join-Path $root 'capture-dimensions.json')
    Get-Content (Join-Path $root 'stdout.log') | Select-String '\[benchmark\] (adapter|avg_fps|gpu_avg_ms)'
    "Evidence: $root"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $prior[$name]) }
}
