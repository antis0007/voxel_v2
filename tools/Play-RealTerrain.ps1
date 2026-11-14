param([string]$Executable)
if ($Executable) { throw 'Arbitrary terrain executables are retired. Update the canonical profile with a verified binding.' }
& (Join-Path $PSScriptRoot 'Play-Axiom.ps1') -Mode RealTerrain
