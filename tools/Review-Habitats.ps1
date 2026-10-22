param(
    [ValidateSet('duel','pressure','farm','matron-farm','region')][string]$Mode='farm',
    [ValidateRange(20,900)][int]$Seconds=105,
    [string]$Label='habitat',
    [string]$Executable='target/release-fast/minecraft_style_voxel_prototype.exe'
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$exe=if([IO.Path]::IsPathRooted($Executable)){$Executable}else{Join-Path $repo $Executable}
$root=Join-Path $repo "target/habitats/$Label"
if(Test-Path -LiteralPath $root){throw "Evidence exists: $root"}
if(-not(Test-Path -LiteralPath $exe)){throw "Executable absent: $exe"}
New-Item -ItemType Directory -Path $root | Out-Null
$names=@('VOXEL_REVIEW_ENEMIES','VOXEL_HABITAT_REVIEW','VOXEL_CAPTURE_DIR','VOXEL_SAVE_DIR')
$previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
    $env:VOXEL_REVIEW_ENEMIES='duel'
    $env:VOXEL_HABITAT_REVIEW=$Mode
    $env:VOXEL_CAPTURE_DIR=$root
    $env:VOXEL_SAVE_DIR=Join-Path $root 'disposable-saves'
    $kind=if($Mode -eq 'region'){'hills'}else{'superflat'}
    $arguments=@("--benchmark-world=$kind",'--benchmark-motion=static','--benchmark-width=1280','--benchmark-height=720',
        '--benchmark-view-chunks=4','--benchmark-warmup=3',"--benchmark-duration=$Seconds",'--benchmark-capture-hud',
        '--benchmark-capture-interval=12','--benchmark-weather=clear','--benchmark-review-time=0.5')
    [ordered]@{executable=$exe;sha256=(Get-FileHash -LiteralPath $exe).Hash;mode=$Mode;arguments=$arguments;
        started_utc=[DateTime]::UtcNow.ToString('o');
        note='Native simulation with ordinary input, actual damage, finite feed/mana and production disk checkpoint/reload. Farm fixture starts with a near-ripe pod, 4096 biomass/4096 mana/64 charge in source buffers and two real crates with 512 biomass each; subsequent development rates are unchanged. Region mode uses natural generated terrain. No forced kills, health refills or expected-kill approximations.'
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'manifest.json')
    $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    $null=$process.Handle
    $process.Id | Set-Content -LiteralPath (Join-Path $root 'pid.txt')
    $deadline=[DateTime]::UtcNow.AddSeconds($Seconds*3+180)
    while(-not $process.WaitForExit(1000)){
        if([DateTime]::UtcNow -gt $deadline){
            $process.Kill()
            throw "Owned native review exceeded wall bound; see $root"
        }
    }
    $gameExit=$process.ExitCode
    if($gameExit -ne 0){throw "Native review exited $gameExit; see $root"}
    $log=Join-Path $root 'stdout.log'
    if(-not(Select-String -LiteralPath $log -SimpleMatch '[benchmark] avg_fps=' -Quiet)){throw "Review incomplete: $root"}
    Select-String -LiteralPath $log -Pattern '\[habitat-disk\]|\[benchmark\] avg_fps=' | ForEach-Object {$_.Line}
    if($Mode -eq 'pressure'){
        $lines=Get-Content -LiteralPath $log
        $rakes=@($lines | Select-String -Pattern '^\[matron\] action=Rake ').Count
        $stamps=@($lines | Select-String -Pattern '^\[matron\] action=Stamp ').Count
        $clearance=@{}
        foreach($line in $lines){
            if($line -match '^\[matron\] stamp-release sequence=(\d+) feet_height=([\d.-]+) range=([\d.-]+)'){
                if([double]::Parse($Matches[2],[Globalization.CultureInfo]::InvariantCulture) -ge 0.65 -and
                   [double]::Parse($Matches[3],[Globalization.CultureInfo]::InvariantCulture) -lt 3.4){$clearance[$Matches[1]]=$true}
            }
        }
        $dodges=0
        foreach($line in $lines){
            if($line -match '^\[matron\] stamp-complete sequence=(\d+) hit=false' -and $clearance.ContainsKey($Matches[1])){$dodges++}
        }
        [ordered]@{rake_commitments=$rakes;stamp_commitments=$stamps;close_range_jump_counters=$dodges} |
            ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'combat-receipt.json')
        if($rakes -eq 0 -or $stamps -eq 0 -or $dodges -eq 0){throw "Close combat/counter acceptance failed: $root"}
    }
    if($Mode -in @('duel','farm','matron-farm') -and $Seconds -ge 70){
        if(-not(Select-String -LiteralPath $log -Pattern 'habitats_equal=true terrain_equal=true sources_equal=true.*duplicate_xp=false' -Quiet)){
            throw "Checkpoint equality not established: $root"
        }
        if(-not(Select-String -LiteralPath $log -Pattern 'factory_equal=true' -Quiet)){throw "Factory checkpoint equality not established: $root"}
    }
    if($Mode -eq 'farm' -and $Seconds -ge 100){
        if(-not(Select-String -LiteralPath $log -SimpleMatch '[habitat-move] placed source=1' -Quiet)){throw "Source relocation missing: $root"}
        $away=$false;$returned=$false
        foreach($line in Get-Content -LiteralPath $log){
            if($line -match '^\[habitat-live\] t=([\d.]+) source=1 .*distance=([\d.]+)'){
                $time=[double]::Parse($Matches[1],[Globalization.CultureInfo]::InvariantCulture)
                $distance=[double]::Parse($Matches[2],[Globalization.CultureInfo]::InvariantCulture)
                if($time -gt 34 -and $distance -gt 100){$away=$true}
                if($away -and $time -gt 70 -and $distance -lt 7){$returned=$true}
            }
        }
        if(-not $returned){throw "Physical departure/return missing: $root"}
    }
    if($Mode -eq 'matron-farm' -and $Seconds -ge 400){
        if(-not(Select-String -LiteralPath $log -Pattern 'habitat-disk\] loaded.*round=2 mastery=4 max_mana=115' -Quiet)){
            throw "Three-harvest mastery reload missing: $root"
        }
    }
    Write-Output "Evidence: $root"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
