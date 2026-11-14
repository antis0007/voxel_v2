param([switch]$NoBrowser)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    cargo build --offline -p engine_worldgen --bin anchor_studio --profile release-fast --target-dir target/anchor-studio/compiler
    if ($LASTEXITCODE -ne 0) { throw 'Anchor Studio could not build.' }
    $options = if ($NoBrowser) { @() } else { @('--open') }
    node tools/anchor-studio/server.mjs @options
    if ($LASTEXITCODE -ne 0) { throw 'Anchor Studio could not start.' }
} finally {
    Pop-Location
}
