param(
    [ValidateSet('Relay','Steamworks','Standalone')][string]$Transport = 'Steamworks',
    [string]$PrebuiltRoot,
    [ValidateSet('dev','release-fast','release')][string]$Profile = 'release-fast',
    [string]$TargetDir,
    [string]$OutputPath,
    [switch]$Offline
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$keys = @('GNS_PREBUILT_ROOT','GNS_VCPKG_BUILDTREES_ROOT','LIBCLANG_PATH','DOCS_RS')
$previous = @{}
foreach ($key in $keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
Push-Location $repo
try {
    if (-not $TargetDir) { $TargetDir = Join-Path $repo 'target' }
    $TargetDir = [IO.Path]::GetFullPath($TargetDir)
    Remove-Item Env:DOCS_RS -ErrorAction SilentlyContinue
    if ($PrebuiltRoot) { $env:GNS_PREBUILT_ROOT = (Resolve-Path -LiteralPath $PrebuiltRoot).Path }
    $env:GNS_VCPKG_BUILDTREES_ROOT = Join-Path $repo 'target/gns-bt'
    $clang = Join-Path $repo 'target/net-toolchain/clang/native'
    if (-not $env:LIBCLANG_PATH -and (Test-Path -LiteralPath "$clang/libclang.dll")) { $env:LIBCLANG_PATH = $clang }
    $feature=switch($Transport){'Steamworks'{'multiplayer-sdr'} 'Standalone'{'multiplayer-steam'} default{'multiplayer'}}
    $suffix=switch($Transport){'Steamworks'{'sdr'} 'Standalone'{'steam'} default{'relay'}}
    $buildArgs = @('rustc','--locked','-p','minecraft_style_voxel_prototype','--bin','minecraft_style_voxel_prototype',
        '--features',$feature,'--profile',$Profile,'--target-dir',$TargetDir)
    if ($Offline) { $buildArgs += '--offline' }
    & cargo @buildArgs -- -C "extra-filename=-$suffix"
    if ($LASTEXITCODE -ne 0) { throw "Native multiplayer build failed ($LASTEXITCODE)" }
    $directory = if ($Profile -eq 'dev') { 'debug' } else { $Profile }
    $built = Join-Path $TargetDir "$directory/deps/minecraft_style_voxel_prototype-$suffix.exe"
    $candidate = if ($OutputPath) { [IO.Path]::GetFullPath($OutputPath) } else { Join-Path $repo "target/multiplayer-$suffix.exe" }
    Copy-Item -LiteralPath $built -Destination $candidate
    if($Transport -eq 'Steamworks') {
        # The selected build already copied its SDK runtime into OUT_DIR.
        # Avoid resolving unrelated platform dependencies just to locate a DLL.
        $runtimes = @(Get-ChildItem -Path (Join-Path $TargetDir "$directory/build/steamworks-sys-*/out/steam_api64.dll") -File)
        if (!$runtimes.Count) { throw 'Built Steamworks runtime unavailable' }
        $hashes = @($runtimes | Get-FileHash -Algorithm SHA256 | Select-Object -ExpandProperty Hash -Unique)
        if ($hashes.Count -ne 1) { throw 'Ambiguous Steamworks runtimes; use a fresh target directory' }
        Copy-Item -LiteralPath $runtimes[0].FullName -Destination (Join-Path (Split-Path -Parent $candidate) 'steam_api64.dll')
    }
    Get-FileHash -LiteralPath $candidate -Algorithm SHA256
} finally {
    Pop-Location
    foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }
}
