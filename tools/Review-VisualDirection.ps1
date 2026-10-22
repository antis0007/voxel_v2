param(
    [ValidateSet('daylight','clouds','water','materials','canopy','cave','motion')]
    [string]$Case = 'daylight',
    [string]$Label = 'candidate',
    [ValidateSet('hills','superflat','ridged')][string]$World = 'hills',
    [int]$Width = 2560,
    [int]$Height = 1440,
    [int]$Seconds = 40,
    [ValidateRange(0.02,20)] [double]$CaptureHz = 0.2,
    [string]$DebugView = '',
    [switch]$Resize,
    [switch]$HideHud,
    [switch]$LiveClock,
    [string]$Executable = 'target/release-fast/minecraft_style_voxel_prototype.exe',
    # 6 keeps old evidence comparable; draw-cost measurements use the game default, 28.
    [ValidateRange(4,64)] [int]$ViewChunks = 6,
    # Extra renderer flags for A/B runs, e.g. -ExtraArgs '--benchmark-no-multidraw'.
    [string[]]$ExtraArgs = @()
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$exe = (Resolve-Path -LiteralPath $Executable).Path
# A capture taken while cargo/rustc run is a timing baseline polluted by the build.
$busy = Get-Process -Name cargo, rustc -ErrorAction SilentlyContinue
if ($busy) { throw "Build activity running ($($busy.Name -join ', ')); wait for it to finish, then capture." }
# A stale exe silently measures old code. Cargo's dep-info lists exactly the files the exe
# was built from (cfg(test) modules and unused crates excluded, include_bytes assets included).
$depInfo = [IO.Path]::ChangeExtension($exe, '.d')
if (-not (Test-Path -LiteralPath $depInfo)) { throw "No dep-info at $depInfo; rebuild with: cargo build --profile release-fast" }
$depText = (Get-Content -Raw -LiteralPath $depInfo) -replace '\\ ', [char]0
$depList = $depText.Substring($depText.IndexOf(': ') + 2) -split '\s+' |
    Where-Object { $_ } | ForEach-Object { $_ -replace [char]0, ' ' }
$newestSource = $depList | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Get-Item |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ((Get-Item -LiteralPath $exe).LastWriteTime -lt $newestSource.LastWriteTime) {
    throw "Executable is older than $($newestSource.FullName); rebuild with: cargo build --profile release-fast"
}
$root = Join-Path $repo "target/visual-direction/$Label/$Case-$Width-$Height"
if ($DebugView) { $root += "-$DebugView" }
if (Test-Path -LiteralPath $root) { throw "Preserve existing evidence: choose a new Label ($root)." }
New-Item -ItemType Directory -Path $root -Force | Out-Null
# Record which code produced the evidence. The tree is usually dirty, so hash the diff too.
# Windows PowerShell 5 turns native stderr (git's CRLF warnings) into terminating errors
# under 'Stop' when the caller redirects stderr, so git runs under 'Continue'.
$ErrorActionPreference = 'Continue'
$gitHead = (git rev-parse HEAD 2>$null).Trim()
$gitStatus = (git status --porcelain 2>$null) -join "`n"
$gitDiff = (git -c core.safecrlf=false diff HEAD 2>$null) -join "`n"
$ErrorActionPreference = 'Stop'
$stateBytes = [Text.Encoding]::UTF8.GetBytes("$gitStatus`n---`n$gitDiff")
$stateHash = [BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($stateBytes)).Replace('-', '')
$argsList = @("--benchmark-world=$World",'--benchmark-seed=2738958700',
    '--benchmark-weather=partly','--benchmark-review-time=0.5',
    "--benchmark-view-chunks=$ViewChunks",'--benchmark-cloud-divisor=3',"--benchmark-width=$Width","--benchmark-height=$Height",
    '--benchmark-warmup=8',"--benchmark-duration=$Seconds",'--benchmark-capture-frames')
if (-not $HideHud) { $argsList += '--benchmark-capture-hud' }
if (-not $LiveClock) { $argsList += '--benchmark-sky-time=12' }
$interval = (1.0 / $CaptureHz).ToString('0.####', [Globalization.CultureInfo]::InvariantCulture)
$argsList += "--benchmark-capture-interval=$interval"
if ($Resize) { $argsList += '--benchmark-resize-review' }
$argsList += $ExtraArgs
$camera = '0,0,5,0,0.16'
switch ($Case) {
    'clouds' { $camera = '0,0,8,0,0.28' }
    'water' { $camera = '0,0,5,0,-0.20' }
    'materials' { $camera = '0,0,5,0,-0.20' }
    'canopy' { $camera = '44,35,7,-1.0,0.14' }
    # Existing native review scene includes the single-light cave.
    'cave' { $argsList += '--benchmark-torch-review' }
    'motion' { $argsList += '--benchmark-horizon-sweep' }
}
if ($Case -notin @('cave','motion')) { $argsList += "--benchmark-terrain-camera=$camera" }
if ($DebugView) { $argsList += "--benchmark-debug=$DebugView" }
$names = @('VOXEL_CAPTURE_DIR','VOXEL_REVIEW_WAND','VOXEL_REVIEW_WATER','VOXEL_REVIEW_TERRAIN')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:VOXEL_CAPTURE_DIR = $root
    $env:VOXEL_REVIEW_WAND = '2'
    $env:VOXEL_REVIEW_WATER = $null
    $env:VOXEL_REVIEW_TERRAIN = $null
    if ($Case -eq 'water') { $env:VOXEL_REVIEW_WATER = '10' }
    if ($Case -eq 'materials') { $env:VOXEL_REVIEW_TERRAIN = '10' }
    [ordered]@{
        started_utc = [DateTime]::UtcNow.ToString('o'); executable = $exe
        executable_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        git_head = $gitHead; working_tree_sha256 = $stateHash
        arguments = $argsList; camera = $camera; budget_p95_ms = 16.67
        note = 'Actual renderer; generated review scenes, not the missing reference photographs. See logs for adapter/backend and timing support.'
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    $process = Start-Process -FilePath $exe -ArgumentList $argsList -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    # Retain the native handle before polling/Refresh (Windows PowerShell 5).
    $null = $process.Handle
    $peakWorking = 0L; $peakPrivate = 0L
    while (-not $process.WaitForExit(500)) {
        $process.Refresh()
        $peakWorking = [Math]::Max($peakWorking, $process.WorkingSet64)
        $peakPrivate = [Math]::Max($peakPrivate, $process.PrivateMemorySize64)
    }
    [ordered]@{exit_code=$process.ExitCode; peak_working_bytes=$peakWorking; peak_private_bytes=$peakPrivate} |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'process.json')
    if ($null -ne $process.ExitCode -and $process.ExitCode -ne 0) { throw "Renderer exited $($process.ExitCode); inspect $root" }
    if (-not (Select-String -LiteralPath (Join-Path $root 'stdout.log') -SimpleMatch '[benchmark] avg_fps=' -Quiet)) {
        throw "Renderer did not complete its measurement; inspect $root"
    }
    Write-Output "Evidence: $root"
    # Summarize-RendererReview.ps1 reads these lines from summary.txt.
    $summary = @(Select-String -LiteralPath (Join-Path $root 'stdout.log') -Pattern '\[benchmark\]' | ForEach-Object { $_.Line })
    $summary | Set-Content -LiteralPath (Join-Path $root 'summary.txt') -Encoding utf8
    $summary
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
}
