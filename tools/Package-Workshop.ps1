param(
    [string]$Binary = (Join-Path $PSScriptRoot '../target/multiplayer-relay.exe'),
    [string]$Output = (Join-Path $PSScriptRoot '../target/workshop-playtest'),
    [ValidateSet('Relay','Steamworks')][string]$Transport = 'Relay',
    [switch]$Archive
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$Output = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $Output) { throw 'Package directory already exists. Select a fresh -Output directory to preserve its build and saves.' }
if ($Archive -and (Test-Path -LiteralPath "$Output.zip")) { throw 'Archive already exists. Select a fresh -Output directory.' }
$dll = Join-Path (Split-Path -Parent $Binary) 'steam_api64.dll'
if ($Transport -eq 'Steamworks' -and -not (Test-Path -LiteralPath $dll)) { throw "Steamworks runtime missing: $dll" }
New-Item -ItemType Directory -Path $Output -Force | Out-Null
Copy-Item -LiteralPath $Binary -Destination (Join-Path $Output 'Workshop.exe')
if ($Transport -eq 'Steamworks') { Copy-Item -LiteralPath $dll -Destination $Output }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Play-Workshop.ps1') -Destination $Output
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Play-Axiom.ps1') -Destination $Output
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'launch') -Destination $Output -Recurse
$launchManifest = [ordered]@{ schema_version = 1; release_id = 'workshop-package'; profiles = [ordered]@{ Workshop = [ordered]@{ executable = 'Workshop.exe'; binding = 'build.json'; hash_key = 'executable_sha256' } } }
$launchManifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Output 'launch/profiles.v1.json') -Encoding utf8

New-Item -ItemType Directory -Path (Join-Path $Output 'assets') -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $repo 'assets') -File | Copy-Item -Destination (Join-Path $Output 'assets')
foreach ($dir in @('runtime','icons','models','groundcover','tools')) {
    $source = Join-Path $repo "assets/$dir"
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $Output 'assets') -Recurse }
}
# The executable embeds the fonts and viewmodel geometry. Keep the font notices
# with the package and leave the modeling sources in the repository.
$fonts = Join-Path $repo 'assets/fonts'
if (Test-Path -LiteralPath $fonts) {
    $fontNotices = New-Item -ItemType Directory -Path (Join-Path $Output 'assets/fonts')
    Get-ChildItem -LiteralPath $fonts -File |
        Where-Object { $_.Extension -in '.txt', '.md' } |
        Copy-Item -Destination $fontNotices.FullName
}
foreach ($role in @('Host','Join','Server')) {
    $lines = @('@echo off', "powershell -NoProfile -File `"%~dp0Play-Workshop.ps1`" -Role $role -Transport $Transport %*", 'pause')
    [IO.File]::WriteAllLines((Join-Path $Output "$role.cmd"), $lines, [Text.Encoding]::ASCII)
    if ($role -ne 'Server') {
        $lowLines = @('@echo off', "powershell -NoProfile -File `"%~dp0Play-Workshop.ps1`" -Role $role -Transport $Transport -LowSpec %*", 'pause')
        [IO.File]::WriteAllLines((Join-Path $Output "$role-LowSpec.cmd"), $lowLines, [Text.Encoding]::ASCII)
    }
}
Copy-Item -LiteralPath (Join-Path $repo 'docs/playtests/WORKSHOP-QUICKSTART.md') -Destination (Join-Path $Output 'READ-ME.md')
Copy-Item -LiteralPath (Join-Path $repo 'docs/playtests/WORKSHOP.md') -Destination (Join-Path $Output 'BUILD-NOTES.md')
$manifest = [ordered]@{
    created_utc = [DateTime]::UtcNow.ToString('o')
    executable_sha256 = (Get-FileHash -LiteralPath (Join-Path $Output 'Workshop.exe') -Algorithm SHA256).Hash
    seed = 74
    maximum_players = 4
    transport = $Transport
    runtime_acceptance = 'Unverified for this executable; see BUILD-NOTES.md for historical evidence and remaining checks.'
    files = @(Get-ChildItem -LiteralPath $Output -Recurse -File | Sort-Object FullName | ForEach-Object {
        [ordered]@{ path = $_.FullName.Substring($Output.TrimEnd('\','/').Length + 1); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
    })
}
$manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'build.json') -Encoding utf8
Write-Output $Output
if ($Archive) {
    Compress-Archive -LiteralPath $Output -DestinationPath "$Output.zip" -CompressionLevel Optimal
    Get-FileHash -LiteralPath "$Output.zip" -Algorithm SHA256
}
