#requires -Version 7.0
param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][ValidatePattern('^[a-zA-Z0-9_-]+$')][string]$Label,
    [Parameter(Mandatory=$true)][string]$EvidenceRoot,
    [string]$SourceRoot='',
    [string]$ExpectedInputs=''
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$source=if($SourceRoot){[IO.Path]::GetFullPath($SourceRoot)}else{$repo}
$expected=@{}
if($ExpectedInputs){
    $before=Get-Content -LiteralPath $ExpectedInputs -Raw | ConvertFrom-Json
    foreach($entry in $before.files){$expected[$entry.path]=$entry.sha256}
    if($expected.Count -eq 0){throw 'Empty pre-build input manifest'}
}
$exePath=if([IO.Path]::IsPathRooted($Executable)){$Executable}else{Join-Path $repo $Executable}
$exe=Get-Item -LiteralPath $exePath
$base=Join-Path ([IO.Path]::GetFullPath($EvidenceRoot)) $Label
if((Test-Path "$base.zip") -or (Test-Path "$base.json")){throw 'Frozen archive already exists'}
$files=@(Get-ChildItem -LiteralPath @("$source/src","$source/assets") -Recurse -File)
$files+=@(Get-Item -LiteralPath @("$source/Cargo.toml","$source/Cargo.lock","$source/build.rs","$source/.cargo/config.toml"))
$files=@($files|Sort-Object FullName -Unique)
if($ExpectedInputs -and $files.Count -ne $expected.Count){throw 'Build input file count changed since freeze'}
$rows=[Collections.Generic.List[object]]::new()
$zip=[IO.Compression.ZipFile]::Open("$base.zip",[IO.Compression.ZipArchiveMode]::Create)
$hasher=[Security.Cryptography.SHA256]::Create()
try {
    foreach($file in $files){
        # Deny writers while hashing and copying the same open input.
        $sourceStream=[IO.File]::Open($file.FullName,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        try {
            if([IO.File]::GetLastWriteTimeUtc($file.FullName) -gt $exe.LastWriteTimeUtc){throw "Input newer than executable: $($file.FullName)"}
            $name=[IO.Path]::GetRelativePath($source,$file.FullName).Replace('\','/')
            $hash=[Convert]::ToHexString($hasher.ComputeHash($sourceStream))
            if($ExpectedInputs -and (!$expected.ContainsKey($name) -or $expected[$name] -ne $hash)){
                throw "Build input changed since freeze: $name"
            }
            $sourceStream.Position=0
            $entry=$zip.CreateEntry($name,[IO.Compression.CompressionLevel]::Optimal)
            $output=$entry.Open()
            try {$sourceStream.CopyTo($output)} finally {$output.Dispose()}
            $rows.Add([ordered]@{path=$name;sha256=$hash;bytes=$sourceStream.Length})
        } finally {$sourceStream.Dispose()}
    }
} finally {$zip.Dispose();$hasher.Dispose()}
$receipt=[ordered]@{
    utc=[DateTime]::UtcNow.ToString('o')
    revision=(& git -C $repo rev-parse HEAD)
    executable_sha256=(Get-FileHash -LiteralPath $exe.FullName).Hash
    source_archive_sha256=(Get-FileHash -LiteralPath "$base.zip").Hash
    scope='Frozen integrated build inputs and runtime assets, including preserved shared working changes; no player saves or target caches. Each archived input was hashed and copied under the same read lock.'
    inputs_newer_than_executable=0
    source_root=$source
    prebuild_manifest=if($ExpectedInputs){[IO.Path]::GetFullPath($ExpectedInputs)}else{$null}
    prebuild_inputs_matched=if($ExpectedInputs){$true}else{$null}
    files=$rows.ToArray()
}
$receipt|ConvertTo-Json -Depth 6|Set-Content -LiteralPath "$base.json"
[ordered]@{files=$rows.Count;executable_sha256=$receipt.executable_sha256;source_archive_sha256=$receipt.source_archive_sha256;manifest="$base.json"}|ConvertTo-Json
