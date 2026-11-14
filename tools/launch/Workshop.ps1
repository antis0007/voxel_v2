param(
    [ValidateSet('Host','Join','Server')][string]$Role = 'Host',
    [string]$Address,
    [string]$Name,
    [string]$Binary,
    [string]$ProjectRoot,
    [string]$Lobby,
    [switch]$LowSpec,
    [ValidateSet('Relay','Steamworks','Standalone')][string]$Transport = 'Relay'
)
$ErrorActionPreference = 'Stop'
$repo = $ProjectRoot
$root = Split-Path -Parent $Binary
if ($Transport -eq 'Steamworks' -and -not (Test-Path -LiteralPath (Join-Path $root 'steam_api64.dll'))) { throw 'Steamworks runtime missing.' }
if (-not $Name) { $Name = Read-Host 'Player name' }
if ([string]::IsNullOrWhiteSpace($Name)) { throw 'A player name is required.' }
if (-not $Address) {
    $Address = if ($Role -eq 'Join') { Read-Host 'Session code (or steam:ID / IP:port for the selected transport)' }
        elseif ($Transport -eq 'Steamworks') { 'steam' } else { '0.0.0.0:27020' }
}
if ([string]::IsNullOrWhiteSpace($Address)) { throw 'A host address is required.' }
$safeName = $Name -replace '[^a-zA-Z0-9_-]', '_'
$data = Join-Path $root ('data/' + $(if ($Role -eq 'Join') { "player-$safeName" } else { 'world' }))
New-Item -ItemType Directory -Path $data -Force | Out-Null
$lock = $null
$verb = if ($Role -eq 'Join') { 'join' } else { 'host' }
$gameArgs = @('--multiplayer-seed=74', "--multiplayer-name=$Name")
if ($Transport -eq 'Relay') {
    $gameArgs += if ($Role -eq 'Join') { "--multiplayer-join-code=$Address" } else { @('--multiplayer-online=Workshop','--multiplayer-unlisted') }
    if ($Lobby) { $gameArgs += "--multiplayer-lobby=$Lobby" }
} else { $gameArgs += "--multiplayer-$verb=$Address" }
if ($Role -ne 'Join') { $gameArgs += "--multiplayer-save=$(Join-Path $data 'workshop.ron')" }
if ($Role -eq 'Server') { $gameArgs += '--multiplayer-headless' }
if ($LowSpec) { $gameArgs += '--low-spec' }
$keys = @('VOXEL_SAVE_DIR','VOXEL_STEAM_APP_ID','SteamAppId','VOXEL_STEAM_DEDICATED')
$previous = @{}; foreach ($key in $keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
Push-Location $root
try {
    if ($Role -ne 'Join') {
        try { $lock = [IO.File]::Open((Join-Path $data 'session.lock'), [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None) }
        catch { throw 'This world is already open. Close its Host or Server before opening it again.' }
        if (-not (Test-Path -LiteralPath (Join-Path $data 'server.properties'))) {
            @('motd=Workshop','max-players=4','gamemode=survival','difficulty=normal','pvp=false','spawn-protection=0','view-distance=6') |
                Set-Content -LiteralPath (Join-Path $data 'server.properties') -Encoding ascii
        }
    }
    $env:VOXEL_SAVE_DIR = $data
    if ($Transport -eq 'Steamworks') {
        $env:VOXEL_STEAM_APP_ID = '480'; $env:SteamAppId = '480'
        $env:VOXEL_STEAM_DEDICATED = if ($Role -eq 'Server') { '1' } else { '0' }
    }
    Invoke-AxiomProcess -Executable $Binary -Arguments $gameArgs -WorkingDirectory $root
} finally {
    if ($lock) { $lock.Dispose() }
    Pop-Location
    foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }
}
