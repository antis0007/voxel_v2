$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Core.ps1')
function Assert($Condition, $Message) { if (-not $Condition) { throw $Message } }
function MustFail([scriptblock]$Action) { $failed=$false; try { & $Action | Out-Null } catch { $failed=$true }; Assert $failed 'Expected failure did not occur.' }
$root = Join-Path ([IO.Path]::GetTempPath()) ('axiom-launch-tests-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
# The fixture is data, never an executable; all process tests use the injected runner.
$fixture=Join-Path $root 'fixture.dat'; Set-Content -LiteralPath $fixture -Value 'launcher-test-data'
$binding=Join-Path $root 'binding.json'
$hash=(Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash
@{fixture=$hash}|ConvertTo-Json|Set-Content -LiteralPath $binding
$profile=[pscustomobject]@{ executable='fixture.dat';binding='binding.json';hash_key='fixture' }
Assert ((Resolve-AxiomBuild $profile $root) -eq $fixture) 'Relative resolution failed.'
$absolute=[pscustomobject]@{executable=$fixture;binding=$binding;hash_key='fixture'}
Assert ((Resolve-AxiomBuild $absolute $root) -eq $fixture) 'Absolute resolution failed.'
$bad=[pscustomobject]@{executable='missing.dat';binding='binding.json';hash_key='fixture'}
MustFail { Resolve-AxiomBuild $bad $root }
foreach($value in @('{','[]','{}','{"fixture":42}','{"fixture":"bad"}',('{"fixture":"'+('0'*64)+'"}'))) {
 Set-Content -LiteralPath $binding -Value $value
 MustFail { Resolve-AxiomBuild $profile $root }
}
@{fixture=$hash}|ConvertTo-Json|Set-Content -LiteralPath $binding
$before=(Get-Location).Path; $envBefore=[Environment]::GetEnvironmentVariable('AXIOM_LAUNCH_TEST','Process')
$global:axiomObserved=$null
$runner={param($exe,$arguments) $global:axiomObserved=@{exe=$exe;arguments=$arguments;cwd=(Get-Location).Path;value=$env:AXIOM_LAUNCH_TEST}}
$arguments=@('--name=two words','--literal=; $(not-a-command)','--unicode=world')
Invoke-AxiomProcess -Executable $fixture -Arguments $arguments -WorkingDirectory $root -Environment @{AXIOM_LAUNCH_TEST='isolated'} -Runner $runner
Assert ($global:axiomObserved.cwd -eq $root) 'Child cwd was not set.'
Assert ($global:axiomObserved.value -eq 'isolated') 'Child environment was not set.'
Assert (($global:axiomObserved.arguments -join '|') -ceq ($arguments -join '|')) 'Argument boundaries changed.'
Assert ((Get-Location).Path -eq $before) 'Caller cwd not restored.'
Assert ([Environment]::GetEnvironmentVariable('AXIOM_LAUNCH_TEST','Process') -ceq $envBefore) 'Caller environment not restored.'
MustFail { Invoke-AxiomProcess $fixture $arguments $root @{AXIOM_LAUNCH_TEST='failure'} {throw 'injected child failure'} }
Assert ((Get-Location).Path -eq $before) 'Failure cwd restore failed.'
Assert ([Environment]::GetEnvironmentVariable('AXIOM_LAUNCH_TEST','Process') -ceq $envBefore) 'Failure environment restore failed.'
MustFail { Invoke-AxiomProcess $fixture $arguments (Join-Path $root 'absent') @{AXIOM_LAUNCH_TEST='failure'} $runner }
Assert ([Environment]::GetEnvironmentVariable('AXIOM_LAUNCH_TEST','Process') -ceq $envBefore) 'Cwd failure environment restore failed.'
# Replace only the process boundary to exercise real mode argument, lock and save planning.
function Invoke-AxiomProcess { param($Executable,$Arguments,$WorkingDirectory) $global:axiomObserved=@{exe=$Executable;arguments=$Arguments;cwd=$WorkingDirectory;save=$env:VOXEL_SAVE_DIR} }
$saveBefore=$env:VOXEL_SAVE_DIR
& (Join-Path $PSScriptRoot 'Workshop.ps1') -Binary $fixture -ProjectRoot $root -Role Host -Transport Relay -Name 'QA User' -Lobby 'room with spaces' -LowSpec
Assert ($global:axiomObserved.arguments -contains '--multiplayer-lobby=room with spaces') 'Workshop lobby argument lost.'
Assert ($global:axiomObserved.arguments -contains '--multiplayer-online=Workshop') 'Workshop host transport changed.'
Assert ($global:axiomObserved.arguments -contains '--low-spec') 'Workshop low-spec lost.'
Assert ($global:axiomObserved.save -eq (Join-Path $root 'data/world')) 'Workshop host save changed.'
Assert ($env:VOXEL_SAVE_DIR -ceq $saveBefore) 'Workshop environment not restored.'
& (Join-Path $PSScriptRoot 'Workshop.ps1') -Binary $fixture -ProjectRoot $root -Role Join -Transport Relay -Name 'QA User' -Address 'session-code'
Assert ($global:axiomObserved.arguments -contains '--multiplayer-join-code=session-code') 'Workshop join changed.'
Assert ($global:axiomObserved.save -eq (Join-Path $root 'data/player-QA_User')) 'Workshop player path changed.'
& (Join-Path $PSScriptRoot 'Multiplayer.ps1') -Binary $fixture -ProjectRoot $root -Role Server -Transport Standalone -Address '127.0.0.1:27020' -Seed 99 -Generator 287 -LowSpec
Assert ($global:axiomObserved.arguments -contains '--multiplayer-headless') 'Server flag lost.'
Assert ($global:axiomObserved.arguments -contains '--multiplayer-seed=99') 'Seed lost.'
Assert ($global:axiomObserved.arguments -contains '--multiplayer-generator=287') 'Generator lost.'
Assert ((Get-Location).Path -eq $before) 'Mode cwd not restored.'
Get-ChildItem -LiteralPath (Split-Path -Parent $PSScriptRoot) -Filter 'Play-*.ps1' | ForEach-Object {
 $tokens=$null;$errors=$null;[Management.Automation.Language.Parser]::ParseFile($_.FullName,[ref]$tokens,[ref]$errors)|Out-Null
 Assert ($errors.Count -eq 0) ('Parse failure: '+$_.Name)
}
"PASS: manifest/hash failures, absolute/relative paths, argument boundaries, working directory, environment restoration, Workshop host/join, Multiplayer server and launcher syntax. Fixtures retained: $root"
