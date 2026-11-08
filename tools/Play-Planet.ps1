param(
    [switch]$Rebuild,
    [ValidateSet('release-fast','release')][string]$Profile = 'release-fast',
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$GameArgs
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$target = if ($Profile -eq 'release') { 'target' } else { 'target/planet142-build' }
$candidate = Join-Path $repo "$target/$Profile/minecraft_style_voxel_prototype.exe"
Push-Location $repo
try {
    if ($Rebuild -or -not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        # Keep this candidate separate from concurrent frozen-source reviews.
        & cargo.exe build --locked --offline -p minecraft_style_voxel_prototype --profile $Profile --bin minecraft_style_voxel_prototype --target-dir $target
        if ($LASTEXITCODE -ne 0) { throw 'Planet build failed.' }
    }
    & $candidate @GameArgs
    if ($LASTEXITCODE -ne 0) { throw "Planet exited with code $LASTEXITCODE." }
} finally {
    Pop-Location
}
