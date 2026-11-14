param([switch]$NoBrowser)

$ErrorActionPreference = 'Stop'
$node = Get-Command node -ErrorAction SilentlyContinue
if (-not $node) { throw 'Axiom Dev Panel needs Node.js 22 or newer.' }
$server = Join-Path $PSScriptRoot 'axiom-dev/server.mjs'
$browser = if ($NoBrowser) { @() } else { @('--open') }
& $node.Source $server @browser
exit $LASTEXITCODE

