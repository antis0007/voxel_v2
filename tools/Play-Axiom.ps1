[CmdletBinding()]
param(
    [ValidateSet('Game','Workshop','Multiplayer','RealTerrain')][string]$Mode = 'Game',
    [ValidateSet('Current','Workshop','Steamworks','Standalone')][string]$Profile,
    [switch]$IsolatedSaveQA,
    [switch]$ValidateOnly,
    [ValidateSet('Host','Join','Server')][string]$Role = 'Host',
    [ValidateSet('Relay','Steamworks','Standalone')][string]$Transport,
    [string]$Address, [string]$Name, [string]$Binary, [string]$Lobby,
    [uint32]$AppId = 480, [switch]$ForceSdr, [uint32]$Seed = 74,
    [Nullable[uint32]]$Generator = $null, [switch]$LowSpec,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$GameArgs
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'launch/Core.ps1')
$root = Split-Path -Parent $PSScriptRoot
if (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'Workshop.exe')) { $root = $PSScriptRoot }
if (-not $Transport) { $Transport = if ($Mode -eq 'Multiplayer') { 'Steamworks' } else { 'Relay' } }
if (-not $Profile) { $Profile = switch ($Mode) { 'Workshop' { 'Workshop' } 'Multiplayer' { $Transport } default { 'Current' } } }
if ($Mode -eq 'Multiplayer' -and $Transport -eq 'Relay') { throw 'For relay sessions use -Mode Workshop.' }
if ($Mode -eq 'Workshop' -and $Profile -ne 'Workshop') { throw 'Workshop mode requires the Workshop profile; use -Binary for a bound external package.' }
if ($Mode -eq 'Multiplayer' -and $Profile -ne $Transport) { throw 'Multiplayer profile must match Transport.' }
if ($Mode -in @('Game','RealTerrain') -and $Profile -ne 'Current') { throw 'Game modes require Current profile.' }
if ($Binary -and $Mode -ne 'Workshop') { throw '-Binary is supported only for a Workshop package with adjacent build.json.' }
if ($IsolatedSaveQA -and $Mode -in @('Workshop','Multiplayer')) { throw 'QA isolation is for Game/RealTerrain. Multiplayer uses its documented host/player save workflow.' }
if ($GameArgs -and $Mode -in @('Workshop','Multiplayer')) { throw 'Use the named multiplayer/workshop parameters instead of trailing arguments.' }
$config = Read-AxiomProfile -ManifestPath (Join-Path $PSScriptRoot 'launch/profiles.v1.json') -Profile $Profile
$exe = Resolve-AxiomBuild -Profile $config -Root $root -Binary $Binary
if ($ValidateOnly) {
    [pscustomobject]@{ Mode=$Mode; Profile=$Profile; Executable=$exe; WorkingDirectory=$(if ($Mode -eq 'Workshop') { Split-Path -Parent $exe } else { $root }); SHA256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash }
    return
}
switch ($Mode) {
    'Workshop' {
        & (Join-Path $PSScriptRoot 'launch/Workshop.ps1') -Binary $exe -ProjectRoot $root -Role $Role -Transport $Transport -Address $Address -Name $Name -Lobby $Lobby -LowSpec:$LowSpec
    }
    'Multiplayer' {
        & (Join-Path $PSScriptRoot 'launch/Multiplayer.ps1') -Binary $exe -ProjectRoot $root -Role $Role -Transport $Transport -Address $Address -AppId $AppId -ForceSdr:$ForceSdr -Seed $Seed -Generator $Generator -LowSpec:$LowSpec
    }
    default {
        $environment = @{}
        if ($IsolatedSaveQA -or $Mode -eq 'RealTerrain') {
            $qa = Join-Path $PSScriptRoot ('../artifacts/launch-qa/' + [Guid]::NewGuid().ToString('N'))
            New-Item -ItemType Directory -Path $qa -ErrorAction Stop | Out-Null
            $environment = @{ VOXEL_SAVE_DIR = (Join-Path $qa 'saves'); VOXEL_GEOLOGY_CACHE_DIR = (Join-Path $qa 'geology-cache'); VOXEL_RELIEF_CACHE_DIR = (Join-Path $qa 'relief-cache') }
            Write-Warning "Private writes: $qa. The library can still list user worlds; create a new QA world."
        }
        $arguments = @($GameArgs)
        if ($Mode -eq 'RealTerrain') { $arguments = @('--real-terrain') + $arguments }
        if ($LowSpec) { $arguments += '--low-spec' }
        Invoke-AxiomProcess -Executable $exe -Arguments $arguments -WorkingDirectory $root -Environment $environment
    }
}
