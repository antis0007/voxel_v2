param(
    [ValidateSet('Steamworks','Standalone')][string]$Transport = 'Steamworks',
    [ValidateSet('Host', 'Join', 'Server')][string]$Role = 'Host',
    [string]$Address,
    [uint32]$AppId = 480,
    [switch]$ForceSdr,
    [uint32]$Seed = 74,
    [Nullable[uint32]]$Generator = $null,
    [switch]$LowSpec
)
& (Join-Path $PSScriptRoot 'Play-Axiom.ps1') -Mode Multiplayer @PSBoundParameters
