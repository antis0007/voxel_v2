function Get-AxiomPath {
    param([string]$Root, [string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) { throw 'Manifest path is missing.' }
    if ([IO.Path]::IsPathRooted($Path)) { return [IO.Path]::GetFullPath($Path) }
    [IO.Path]::GetFullPath((Join-Path $Root $Path))
}
function Read-AxiomProfile {
    param([string]$ManifestPath, [string]$Profile)
    $manifest = Get-Content -LiteralPath $ManifestPath -Raw -ErrorAction Stop | ConvertFrom-Json -ErrorAction Stop
    if ($manifest.schema_version -ne 1) { throw 'Unsupported launcher manifest schema.' }
    $entry = $manifest.profiles.PSObject.Properties[$Profile]
    if ($null -eq $entry) { throw "Unknown launch profile: $Profile" }
    $entry.Value
}
function Resolve-AxiomBuild {
    param($Profile, [string]$Root, [string]$Binary)
    $exe = if ($Binary) { [IO.Path]::GetFullPath($Binary) } else { Get-AxiomPath -Root $Root -Path $Profile.executable }
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Release executable missing: $exe. No build was attempted." }
    $binding = if ($Binary) { Join-Path (Split-Path -Parent $exe) 'build.json' } else { Get-AxiomPath -Root $Root -Path $Profile.binding }
    $key = if ($Binary) { 'executable_sha256' } else { $Profile.hash_key }
    $manifest = Get-Content -LiteralPath $binding -Raw -ErrorAction Stop | ConvertFrom-Json -ErrorAction Stop
    if ($manifest -isnot [System.Management.Automation.PSCustomObject]) { throw 'Hash binding must be a JSON object.' }
    $hash = $manifest.PSObject.Properties[$key]
    if ($null -eq $hash -or $hash.Value -isnot [string] -or $hash.Value -cnotmatch '^[a-fA-F0-9]{64}$') { throw 'Missing or malformed SHA256 binding.' }
    if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash -ne $hash.Value) { throw 'Executable hash mismatch. Launch cancelled.' }
    $exe
}
function Invoke-AxiomProcess {
    param([string]$Executable, [string[]]$Arguments, [string]$WorkingDirectory,
        [hashtable]$Environment = @{}, [scriptblock]$Runner)
    $previous = @{}; $pushed = $false
    foreach ($key in $Environment.Keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key,'Process') }
    try {
        foreach ($key in $Environment.Keys) { [Environment]::SetEnvironmentVariable($key,$Environment[$key],'Process') }
        Push-Location -LiteralPath $WorkingDirectory -ErrorAction Stop; $pushed = $true
        if ($Runner) { & $Runner $Executable $Arguments } else {
            & $Executable @Arguments
            if ($LASTEXITCODE -ne 0) { throw "Game exited with code $LASTEXITCODE." }
        }
    } finally {
        if ($pushed) { Pop-Location }
        foreach ($key in $Environment.Keys) { [Environment]::SetEnvironmentVariable($key,$previous[$key],'Process') }
    }
}
