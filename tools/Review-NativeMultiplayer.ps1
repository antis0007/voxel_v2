param(
    [string]$Executable = "$PSScriptRoot/../target/multiplayer-steam.exe",
    [string]$Root = "$PSScriptRoot/../target/native-review-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [int]$Port = 27039,
    [uint32]$Seed = 617,
    [switch]$Steamworks,
    [uint32]$AppId = 480,
    [switch]$Work,
    [switch]$Travel,
    [switch]$Rendezvous,
    [switch]$Punch,
    [switch]$AnonymousDirect,
    [string]$LobbyExecutable = "$PSScriptRoot/../target/release-fast/voxel-lobby.exe",
    [int]$LobbyPort = 27040,
    [ValidateRange(0,5000)][int]$LagMs = 0,
    [ValidateRange(0,100)][double]$LossPct = 0,
    [switch]$RenderClient,
    [ValidatePattern('^[a-zA-Z0-9_-]+@[a-zA-Z0-9.-]+$')][string]$RemoteHost,
    [ValidatePattern('^[a-zA-Z0-9_-]+$')][string]$RemoteDirectory = 'voxel-sdr-review-20261028',
    [switch]$NoPvp
)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Executable).Path
$repo = Split-Path -Parent $PSScriptRoot
if (Test-Path -LiteralPath $Root) { throw 'Use a new review directory to preserve prior evidence.' }
New-Item -ItemType Directory -Path $Root | Out-Null
$Root = (Resolve-Path -LiteralPath $Root).Path
$runLabel=Split-Path -Leaf $Root
if($RemoteHost -and (-not $Steamworks -or $runLabel -notmatch '^[a-zA-Z0-9_-]+$')){throw 'Remote review needs Steamworks and a simple run-directory name'}
if($Rendezvous -and $RemoteHost){throw 'Rendezvous review uses local processes'}
if($Punch -and ($Rendezvous -or $RemoteHost -or $RenderClient)){throw 'Punch review uses only local headless peers'}
if($AnonymousDirect -and (-not $Rendezvous -or -not $Steamworks)){throw 'AnonymousDirect requires a Steamworks rendezvous review'}
Set-Content -LiteralPath "$Root/network-profile.txt" -Value "send_lag_ms=$LagMs send_loss_percent=$LossPct rendezvous=$Rendezvous punch=$Punch"
$children = [Collections.Generic.List[object]]::new()
function Start-Peer($Name, $Arguments) {
    $save = Join-Path $Root $Name
    New-Item -ItemType Directory -Path $save | Out-Null
    if ($Name -eq 'host') { Set-Content -LiteralPath "$save/server.properties" -Value "pvp=$(-not $NoPvp)`nspawn-protection=0" }
    $remote=$RemoteHost -and $Name -eq 'host'
    $info = [Diagnostics.ProcessStartInfo]::new($(if($remote){'ssh'}else{$exe}))
    $info.WorkingDirectory = $repo
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.Environment['VOXEL_SAVE_DIR'] = $save
    if($Punch){$info.Environment['VOXEL_STUN']='off'}
    if($LagMs -gt 0){$info.Environment['VOXEL_NET_LAG_MS'] = "$LagMs"}
    if($LossPct -gt 0){$info.Environment['VOXEL_NET_LOSS_PCT'] = $LossPct.ToString([Globalization.CultureInfo]::InvariantCulture)}
    if($Name -eq 'rendered'){$info.Environment['VOXEL_CAPTURE_DIR']=Join-Path $Root 'captures';$info.WindowStyle='Normal'}
    if($Steamworks){
        $info.Environment['VOXEL_STEAM_APP_ID']="$AppId"
        $info.Environment['SteamAppId']="$AppId"
        $info.Environment['VOXEL_STEAM_FORCE_SDR']=if($Rendezvous -or $Punch){'0'}else{'1'}
        $info.Environment['VOXEL_STEAM_DEDICATED']=if($AnonymousDirect -or ($Name -eq 'host' -and -not ($Rendezvous -or $Punch))){'1'}else{'0'}
    }
    if($remote){
        foreach($arg in @('-T','-o','PreferredAuthentications=password','-o','PubkeyAuthentication=no','-o','ConnectTimeout=12',$RemoteHost)){$info.ArgumentList.Add($arg)}
        $properties=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("pvp=$(-not $NoPvp)`nspawn-protection=0`n"))
        $remoteArgs=($Arguments | Where-Object {$_ -ne '--multiplayer-headless' -and $_ -notlike '--multiplayer-host=*'}) -join ' '
        $command='cd "$HOME/'+$RemoteDirectory+'" && mkdir -p runs/'+$runLabel+'/saves && printf %s '+$properties+' | base64 -d > runs/'+$runLabel+'/saves/server.properties && VOXEL_SAVE_DIR="$PWD/runs/'+$runLabel+'/saves" VOXEL_STEAM_FORCE_SDR=1 /usr/bin/time -v -o runs/'+$runLabel+'/metrics.txt timeout 240 bash ./run-steam-server.sh '+$remoteArgs
        $info.ArgumentList.Add($command)
    }else{
        $info.Environment.Remove('VOXEL_RELAY_PASSWORD') | Out-Null
        foreach ($arg in $Arguments) { $info.ArgumentList.Add($arg) }
    }
    $Arguments | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Root "$Name-arguments.json")
    $process = [Diagnostics.Process]::Start($info)
    $peer = [pscustomobject]@{Name=$Name; Process=$process; Pending=$process.StandardError.ReadLineAsync();
        Output=$process.StandardOutput.ReadToEndAsync(); Lines=[Collections.Generic.List[string]]::new();
        Log=(Join-Path $Root "$Name.log"); Ended=$false}
    $children.Add($peer)
    return $peer
}
function Pump {
    foreach ($peer in $children) {
        while (-not $peer.Ended -and $peer.Pending.IsCompleted) {
            $line = $peer.Pending.GetAwaiter().GetResult()
            if ($null -eq $line) { $peer.Ended=$true; break }
            $peer.Lines.Add($line)
            [IO.File]::AppendAllText($peer.Log, "$line`n")
            $peer.Pending = $peer.Process.StandardError.ReadLineAsync()
        }
    }
}
function Has-Line($Peer, $Pattern) { return [bool]($Peer.Lines | Select-String -Pattern $Pattern -Quiet) }
function Submit-PunchAnswer($Peer) {
    $answer=$Peer.Lines | Select-String 'punch answer=(\S+)' | Select-Object -Last 1
    if($answer -and $answers.Add($Peer.Name)){
        $hostPeer.Process.StandardInput.WriteLine($answer.Matches[0].Groups[1].Value)
    }
}
$common = @('--multiplayer-headless','--multiplayer-trace')
$directoryProcess = $null
try {
    $hostAddress=if($Steamworks){'steam'}else{"127.0.0.1:$Port"}
    $listenPattern='\[multiplayer\] listening'
    $hosting = @("--multiplayer-host=$hostAddress")
    if($Punch){
        $hosting=@('--multiplayer-host=punch')
        $listenPattern='\[multiplayer\] punch offer='
    }elseif($Rendezvous){
        $lobbyAddress="127.0.0.1:$LobbyPort"
        $directoryLog=Join-Path $Root 'rendezvous.log'
        $directoryProcess=Start-Process -FilePath (Resolve-Path -LiteralPath $LobbyExecutable).Path -ArgumentList @('--bind',$lobbyAddress) -WindowStyle Hidden -PassThru -RedirectStandardError $directoryLog -RedirectStandardOutput (Join-Path $Root 'rendezvous.out')
        $deadline=[DateTime]::UtcNow.AddSeconds(10)
        while(-not (Select-String -LiteralPath $directoryLog -Pattern '\[lobby\] listening' -Quiet -ErrorAction SilentlyContinue)){
            if($directoryProcess.HasExited -or [DateTime]::UtcNow -gt $deadline){throw 'Rendezvous did not listen'}
            Start-Sleep -Milliseconds 50
        }
        $hosting=@('--multiplayer-online=DirectReview',"--multiplayer-lobby=$lobbyAddress","--multiplayer-udp-port=$Port")
        $listenPattern='\[multiplayer\] listed code='
    }
    $hostArgs=$common+$hosting+@("--multiplayer-seed=$Seed",'--multiplayer-ephemeral','--multiplayer-review-arena','--multiplayer-run-seconds=180')
    if($Travel){$hostArgs+='--multiplayer-review-travel'}
    $hostPeer=Start-Peer 'host' $hostArgs
    $deadline=[DateTime]::UtcNow.AddSeconds($(if($RemoteHost -or $Rendezvous -or $Punch){150}else{50}))
    while (-not (Has-Line $hostPeer $listenPattern)) {
        Pump
        if ($hostPeer.Process.HasExited -or [DateTime]::UtcNow -gt $deadline) { throw 'Native host failed to listen; see host.log.' }
        Start-Sleep -Milliseconds 50
    }
    $joinAddress="127.0.0.1:$Port"
    $joinArgs=@("--multiplayer-join=$joinAddress")
    if($Punch){
        $offer=$hostPeer.Lines | Select-String 'punch offer=(\S+)' | Select-Object -Last 1
        $joinArgs=@("--multiplayer-join-code=$($offer.Matches[0].Groups[1].Value)")
    }elseif($Rendezvous){
        $listen=$hostPeer.Lines | Select-String 'listed code=(\S+)' | Select-Object -Last 1
        $joinArgs=@("--multiplayer-join-code=$($listen.Matches[0].Groups[1].Value)","--multiplayer-lobby=$lobbyAddress")
    }elseif($Steamworks){
        $listen=$hostPeer.Lines | Select-String '\[steamworks\] listening steam:(\d+)' | Select-Object -Last 1
        if(-not $listen){throw 'Steam P2P listener identity missing'}
        $joinAddress='steam:'+$listen.Matches[0].Groups[1].Value
        $joinArgs=@("--multiplayer-join=$joinAddress")
    }
    $idleStart=$hostPeer.Process.TotalProcessorTime.TotalSeconds
    $idleUntil=[DateTime]::UtcNow.AddSeconds(3)
    while ([DateTime]::UtcNow -lt $idleUntil) { Pump; Start-Sleep -Milliseconds 25 }
    $hostPeer.Process.Refresh()
    $idleCpu=$hostPeer.Process.TotalProcessorTime.TotalSeconds-$idleStart
    Set-Content -LiteralPath "$Root/idle-cpu.txt" -Value $(if($RemoteHost){'Remote host: local SSH CPU is not a server CPU measurement. See remote metrics.txt.'}else{"idle_cpu_seconds=$idleCpu wall_seconds=3"})
    $bot=if($Travel){'--multiplayer-bot-travel'}elseif($Work){'--multiplayer-bot-work'}else{'--multiplayer-bot-pvp'}
    $duration=if($Travel){75}else{35}
    $answers=[Collections.Generic.HashSet[string]]::new()
    $clientA=Start-Peer 'a' ($common+$joinArgs+@('--multiplayer-name=SteamA',$bot,"--multiplayer-run-seconds=$duration"))
    $clientB=Start-Peer 'b' ($common+$joinArgs+@('--multiplayer-name=SteamB','--multiplayer-audio','--multiplayer-bot-respawn',"--multiplayer-run-seconds=$duration"))
    $visual=$null
    if($RenderClient){$visual=Start-Peer 'rendered' ($joinArgs+@('--multiplayer-name=WanViewer','--benchmark-warmup=1','--benchmark-duration=10','--benchmark-view-chunks=4','--benchmark-width=960','--benchmark-height=540','--benchmark-capture-hud','--benchmark-capture-interval=2','--low-spec'))}
    $sent=$false
    $travelSteps=@('trade confirmed','fishing catch confirmed','mounted','steered','dismounted')
    $travelEnded=$false
    $deadline=[DateTime]::UtcNow.AddSeconds($(if($Rendezvous -or $Travel -or $Punch){125}elseif($Steamworks){85}else{45}))
    while (-not ($clientA.Process.HasExited -and $clientB.Process.HasExited -and ($null -eq $visual -or $visual.Process.HasExited))) {
        Pump
        if($Punch){
            # The idle guest claims the first spawn as soon as its snapshot is made.
            if(-not $Travel -or (Has-Line $hostPeer '\[multiplayer\] snapshot id=')){
                Submit-PunchAnswer $clientA
            }
            Submit-PunchAnswer $clientB
        }
        if($Travel -and (Has-Line $clientA '\[travel-review\] trade refused:')){
            throw ($clientA.Lines | Select-String '\[travel-review\] trade refused:' | Select-Object -Last 1).Line
        }
        if (-not $sent -and (Has-Line $clientA 'synchronized') -and (Has-Line $clientB 'synchronized')) {
            if($Rendezvous){
                foreach($peer in @($clientA,$clientB)){
                    if(-not (Has-Line $peer 'invite resolved; connecting directly')){throw 'Join did not negotiate a direct UDP address'}
                }
                $directoryProcess.Kill()
                if(-not $directoryProcess.WaitForExit(5000)){throw 'Rendezvous did not stop'}
                Set-Content -LiteralPath "$Root/rendezvous-stopped.txt" -Value ([DateTime]::UtcNow.ToString('o'))
            }
            $hostPeer.Process.StandardInput.WriteLine('op SteamA')
            $clientA.Process.StandardInput.WriteLine('native-public-chat')
            $clientA.Process.StandardInput.WriteLine('/msg SteamB native-private-chat')
            $clientB.Process.StandardInput.WriteLine('/list')
            $clientB.Process.StandardInput.WriteLine('/time set 6000')
            $sent=$true
        }
        if($Travel -and $sent -and -not $travelEnded -and (Has-Line $clientB 'native-private-chat') -and (Has-Line $clientB 'only an operator')){
            $missing=@($travelSteps | Where-Object { -not (Has-Line $clientA ('\[travel-review\] '+$_)) })
            if($missing.Count -eq 0){
                $clientA.Process.StandardInput.WriteLine('quit')
                $clientB.Process.StandardInput.WriteLine('quit')
                $travelEnded=$true
            }
        }
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Native client run timed out.' }
        Start-Sleep -Milliseconds 25
    }
    Pump
    $hostPeer.Process.StandardInput.WriteLine('quit')
    if (-not $hostPeer.Process.WaitForExit($(if($RemoteHost){45000}else{15000}))) { throw 'Native host did not shut down.' }
    Pump
    foreach ($peer in $children) {
        if ($peer.Process.ExitCode -ne 0) { throw "$($peer.Name) exited $($peer.Process.ExitCode); see its log." }
    }
    if (-not $sent) { throw 'Both clients must finish synchronization.' }
    if($Punch -and ($answers.Count -ne 2 -or -not (Has-Line $clientA 'punched direct UDP path') -or -not (Has-Line $clientB 'punched direct UDP path'))){
        throw 'Both guests must complete direct offer and answer exchange'
    }
    if (-not (Has-Line $clientB 'native-public-chat')) { throw 'Public chat was not delivered.' }
    if (-not (Has-Line $clientB 'native-private-chat')) { throw 'Private chat was not delivered.' }
    if (-not (Has-Line $clientB 'only an operator')) { throw 'Non-operator command was not refused.' }
    if ($NoPvp) {
        if (Has-Line $hostPeer '\[pvp\]') { throw 'PvP damage occurred with pvp=false.' }
    } else {
        if (-not (Has-Line $hostPeer '\[pvp\].*health=0\.0')) { throw 'PvP did not reach a player death.' }
        if (-not (Has-Line $clientB 'requesting respawn')) { throw 'Victim did not request a real respawn.' }
        $afterRespawn = $false
        $revived = $false
        foreach ($line in $clientB.Lines) {
            if ($line -match 'requesting respawn') { $afterRespawn = $true }
            if ($afterRespawn -and $line -match '\[network\] ready=true.*health=[1-9]') { $revived = $true }
        }
        if (-not $revived) { throw 'Host did not restore the victim after respawn.' }
        if (-not (Has-Line $clientB '\[network\] sounds=')) { throw 'Remote combat audio was not delivered.' }
    }
    if($Work){
        if(-not (Has-Line $clientA 'items=.*Dirtx[1-9]')){throw 'Mined inventory was not replicated'}
        if(-not (Has-Line $clientA 'craft recipe=')){throw 'Craft action was not attempted'}
        if(-not (Has-Line $hostPeer 'command_ack=[1-9]')){throw 'Inventory action was not acknowledged by the host'}
    }
    if($Travel){
        foreach($step in $travelSteps){
            if(-not (Has-Line $clientA ('\[travel-review\] '+$step))){throw "Travel result missing: $step"}
        }
    }
    if($RenderClient -and -not (Get-ChildItem "$Root/captures" -Filter '*.png' -ErrorAction SilentlyContinue)){throw 'Rendered WAN capture missing'}
    $metrics=$children | ForEach-Object { [pscustomobject]@{peer=$_.Name; scope=if($RemoteHost -and $_.Name -eq 'host'){'ssh-client-only'}else{'local-game'}; cpu_seconds=$_.Process.TotalProcessorTime.TotalSeconds; exit=$_.Process.ExitCode} }
    $metrics | ConvertTo-Json | Set-Content -LiteralPath "$Root/process-metrics.json"
    "PASS native host/join, chat, commands, PvP=$(-not $NoPvp). Evidence: $Root"
} finally {
    if($null -ne $directoryProcess){
        if(-not $directoryProcess.HasExited){$directoryProcess.Kill()}
        $directoryProcess.Dispose()
    }
    foreach ($peer in $children) {
        if (-not $peer.Process.HasExited) {
            try { $peer.Process.StandardInput.WriteLine('quit') } catch {}
            if (-not $peer.Process.WaitForExit(5000)) { $peer.Process.Kill() }
        }
        $peer.Process.Dispose()
    }
}
