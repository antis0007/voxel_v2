param(
    [string]$Binary,
    [string]$ProjectRoot,
    [ValidateSet('Steamworks','Standalone')][string]$Transport = 'Steamworks',
    [ValidateSet('Host', 'Join', 'Server')][string]$Role = 'Host',
    [string]$Address,
    [uint32]$AppId = 480,
    [switch]$ForceSdr,
    [uint32]$Seed = 74,
    [Nullable[uint32]]$Generator = $null,
    [switch]$LowSpec
)
$ErrorActionPreference = 'Stop'
$repo = $ProjectRoot
$candidate = $Binary
if ($Transport -eq 'Steamworks' -and -not (Test-Path -LiteralPath (Join-Path (Split-Path -Parent $Binary) 'steam_api64.dll'))) { throw 'Steamworks runtime missing.' }
$verb = if ($Role -eq 'Join') { 'join' } else { 'host' }
if(-not $Address){$Address=if($Transport -eq 'Steamworks'){'steam'}else{'127.0.0.1:27020'}}
if($Role -eq 'Join' -and $Address -eq 'steam'){throw 'Join needs -Address steam:<host SteamID64>'}
$gameArgs = @("--multiplayer-$verb=$Address", "--multiplayer-seed=$Seed")
if ($null -ne $Generator) { $gameArgs += "--multiplayer-generator=$Generator" }
if ($LowSpec) { $gameArgs += '--low-spec' }
if ($Role -eq 'Server') { $gameArgs += '--multiplayer-headless' }
Push-Location $repo
$keys=@('VOXEL_STEAM_APP_ID','SteamAppId','VOXEL_STEAM_FORCE_SDR','VOXEL_STEAM_DEDICATED')
$previous=@{};foreach($key in $keys){$previous[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
    if($Transport -eq 'Steamworks') {
        $env:VOXEL_STEAM_APP_ID="$AppId";$env:SteamAppId="$AppId"
        $env:VOXEL_STEAM_FORCE_SDR=if($ForceSdr){'1'}else{'0'}
        $env:VOXEL_STEAM_DEDICATED=if($Role -eq 'Server'){'1'}else{'0'}
    }
    Invoke-AxiomProcess -Executable $candidate -Arguments $gameArgs -WorkingDirectory $repo
} finally {
    foreach($key in $keys){[Environment]::SetEnvironmentVariable($key,$previous[$key],'Process')}
    Pop-Location
}
