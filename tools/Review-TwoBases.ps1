param(
    [string]$Executable = "$PSScriptRoot/../target/multiplayer-relay.exe",
    [string]$Relay = "$PSScriptRoot/../target/release-fast/voxel-lobby.exe",
    [string]$Root = "$PSScriptRoot/../target/two-base-review-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [int]$Port = 27047,
    [switch]$Punch,
    [switch]$TraceWorld
)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Executable).Path
$relayExe = if (!$Punch) { (Resolve-Path -LiteralPath $Relay).Path }
$repo = Split-Path -Parent $PSScriptRoot
if (Test-Path -LiteralPath $Root) { throw 'Choose a fresh review directory.' }
$Root = (New-Item -ItemType Directory -Path $Root).FullName
Get-FileHash -LiteralPath $exe | ConvertTo-Json | Set-Content "$Root/executable.json"
$children = [Collections.Generic.List[object]]::new()
$address = "127.0.0.1:$Port"
$common = @('--multiplayer-headless','--multiplayer-trace','--multiplayer-review-bases','--multiplayer-run-seconds=180')
function Start-Peer($Label, $Profile, $Arguments) {
    $save = Join-Path $Root $Profile
    if (!(Test-Path -LiteralPath $save)) { New-Item -ItemType Directory -Path $save | Out-Null }
    $info = [Diagnostics.ProcessStartInfo]::new($exe)
    $info.WorkingDirectory = $repo
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.Environment['VOXEL_SAVE_DIR'] = $save
    if ($Punch) {
        $info.Environment['VOXEL_STUN'] = 'off'
        $info.Environment['VOXEL_STEAM_DEDICATED'] = '0'
        $info.Environment['VOXEL_STEAM_FORCE_SDR'] = '0'
    }
    if ($Label -eq 'host' -and $TraceWorld) { $info.Environment['VOXEL_GAMEPLAY_TRACE'] = '1' }
    $info.Environment.Remove('VOXEL_RELAY_PASSWORD') | Out-Null
    foreach ($arg in ($common + $Arguments)) { $info.ArgumentList.Add($arg) }
    $p = [Diagnostics.Process]::Start($info)
    $peer = [pscustomobject]@{ Label=$Label; Process=$p; Pending=$p.StandardError.ReadLineAsync()
        Output=$p.StandardOutput.ReadToEndAsync(); Lines=[Collections.Generic.List[string]]::new(); Ended=$false }
    $children.Add($peer)
    $peer
}
function Pump {
    foreach ($peer in $children) {
        while (!$peer.Ended -and $peer.Pending.IsCompleted) {
            $line = $peer.Pending.GetAwaiter().GetResult()
            if ($null -eq $line) { $peer.Ended = $true; break }
            $peer.Lines.Add($line)
            [IO.File]::AppendAllText("$Root/$($peer.Label).log", "$line`n")
            $peer.Pending = $peer.Process.StandardError.ReadLineAsync()
        }
    }
}
function Wait-For($Peer, [string]$Pattern, [int]$Since = 0, [int]$Seconds = 30) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        Pump
        for ($i = $Since; $i -lt $Peer.Lines.Count; $i++) {
            if ($Peer.Lines[$i] -match $Pattern) { return $Peer.Lines[$i] }
        }
        if ($Peer.Process.HasExited) { throw "$($Peer.Label) exited before '$Pattern'." }
        Start-Sleep -Milliseconds 60
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$($Peer.Label) did not report '$Pattern'. See $Root."
}
function Send($Peer, $Line) {
    $Peer.Process.StandardInput.WriteLine($Line)
    $Peer.Process.StandardInput.Flush()
}
function Join-Base($Label, $Profile, $Name) {
    $peer = Start-Peer $Label $Profile ($joinArgs + "--multiplayer-name=$Name")
    if ($Punch) {
        $line = Wait-For $peer 'punch answer=(\S+)'
        Send $hostPeer ([regex]::Match($line, 'punch answer=(\S+)').Groups[1].Value)
        Wait-For $peer 'punched direct UDP path' | Out-Null
    }
    $peer
}
function Wait-CraterAndStock($Peer, [int]$Minimum) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $since = 0
    do {
        $line = Wait-For $Peer 'outpost_iron=(\d+) blast=0' $since 5
        $units = [int]([regex]::Match($line, 'outpost_iron=(\d+)').Groups[1].Value)
        if ($units -ge $Minimum) { return }
        $since = $Peer.Lines.IndexOf($line, $since) + 1
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$($Peer.Label) did not converge on crater and delivered stock."
}
function Teleport($Peer, $Name, $X) {
    $since = $Peer.Lines.Count
    Send $hostPeer "tp $Name $X 192.1 1"
    # The correction is asynchronous. Confirm the client's subsequent pose,
    # accepting the fixture's supported voxel scales.
    $cellX = [regex]::Escape(([double]$X / $voxel).ToString([Globalization.CultureInfo]::InvariantCulture))
    Wait-For $Peer "^\[network\] ready=true .* at=Vec3\($cellX(?:\.0)?, " $since | Out-Null
}
try {
    $lobby = "--multiplayer-lobby=$address"
    if ($Punch) {
        $hostArgs = @('--multiplayer-host=punch')
        $invitePattern = 'punch offer=(\S+)'
    } else {
        $relayPeer = Start-Process -FilePath $relayExe -ArgumentList '--bind',$address -WindowStyle Hidden -PassThru
        $hostArgs = @('--multiplayer-online=BaseReview',$lobby,'--multiplayer-unlisted')
        $invitePattern = 'listed code=([A-Z0-9-]+)'
    }
    $hostPeer = Start-Peer 'host' 'host' ($hostArgs + @('--multiplayer-ephemeral','--multiplayer-seed=617'))
    $fixture = Wait-For $hostPeer '\[base-review\] fixture voxel_m=' 0 55
    $voxel = [double]::Parse(($fixture -split '=')[-1], [Globalization.CultureInfo]::InvariantCulture)
    $listed = Wait-For $hostPeer $invitePattern
    $joinArgs = @("--multiplayer-join-code=$([regex]::Match($listed, $invitePattern).Groups[1].Value)")
    if (!$Punch) { $joinArgs += $lobby }
    $a = Join-Base 'a' 'a' 'BaseA'
    $b = Join-Base 'b' 'b' 'BaseB'
    foreach ($peer in @($a, $b)) { Wait-For $peer 'synchronized' 0 65 | Out-Null }
    Teleport $a 'BaseA' 0
    Teleport $b 'BaseB' 384
    $split = Wait-For $hostPeer 'home_iron=[1-9]\d* outpost_iron=[1-9]\d* blast=[1-9]\d*'
    foreach ($peer in @($a, $b)) {
        Wait-For $peer 'home_iron=[1-9]\d* outpost_iron=[1-9]\d* blast=[1-9]\d*' | Out-Null
    }
    Teleport $b 'BaseB' 0
    $before = Wait-For $hostPeer 'outpost_iron=\d+' $hostPeer.Lines.Count
    $initial = [int]([regex]::Match($before, 'outpost_iron=(\d+)').Groups[1].Value)
    $samples = 0
    do {
        $after = Wait-For $hostPeer 'outpost_iron=\d+' $hostPeer.Lines.Count 10
        $later = [int]([regex]::Match($after, 'outpost_iron=(\d+)').Groups[1].Value)
        if ($later -le $initial -and (++$samples -gt 6)) { throw 'Outpost stopped producing.' }
    } until ($later -gt $initial)
    Teleport $b 'BaseB' 384
    Send $hostPeer 'review-blast'
    foreach ($peer in @($hostPeer, $a, $b)) { Wait-CraterAndStock $peer $later }
    Send $b 'quit'
    if (!$b.Process.WaitForExit(8000) -or $b.Process.ExitCode -ne 0) { throw 'Client B did not exit cleanly.' }
    $b2 = Join-Base 'b-rejoin' 'b' 'BaseB'
    Wait-For $b2 'synchronized' 0 65 | Out-Null
    Wait-CraterAndStock $b2 $later
    $remoteCell = [regex]::Escape((384.0 / $voxel).ToString([Globalization.CultureInfo]::InvariantCulture))
    Wait-For $b2 "^\[network\] ready=true .* at=Vec3\($remoteCell(?:\.0)?, " | Out-Null
    Teleport $b2 'BaseB' 0
    Pump
    foreach ($peer in @($hostPeer, $a, $b2)) {
        if ($peer.Process.HasExited) { throw "$($peer.Label) disconnected unexpectedly." }
    }
    @('Two independent native headless clients over localhost',
        "Split: $split", "Unattended outpost iron (1/64 ingot units): $initial → $later",
        'Host and both clients observed the crater; same-profile rejoin observed crater and stock.',
        'No rendered clients, WAN, wireless link, or server restart covered.') |
        Set-Content "$Root/result.txt"
    Get-Content "$Root/result.txt"
} finally {
    foreach ($peer in $children) { if (!$peer.Process.HasExited) { Send $peer 'quit' } }
    foreach ($peer in $children) {
        if (!$peer.Process.WaitForExit(8000)) { $peer.Process.Kill() }
    }
    Pump
    if ($relayPeer -and !$relayPeer.HasExited) { $relayPeer.Kill() }
}
