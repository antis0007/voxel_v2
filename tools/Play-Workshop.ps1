param(
    [ValidateSet('Host','Join','Server')][string]$Role = 'Host',
    [string]$Address,
    [string]$Name,
    [string]$Binary,
    [string]$Lobby,
    [switch]$LowSpec,
    [ValidateSet('Relay','Steamworks','Standalone')][string]$Transport = 'Relay'
)
& (Join-Path $PSScriptRoot 'Play-Axiom.ps1') -Mode Workshop @PSBoundParameters
