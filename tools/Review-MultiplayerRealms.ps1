param(
    [string]$Executable = 'target/release-fast/minecraft_style_voxel_prototype.exe',
    [string]$Root = ('target/realm-review-' + (Get-Date -Format yyyyMMdd-HHmmss)),
    [int]$Port = 27347,
    [switch]$Punch
)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot -Parent)
$root = (New-Item -ItemType Directory -Path $Root).FullName
if ($Punch) {
    $game = (Resolve-Path -LiteralPath $Executable).Path
} else {
    $game = Join-Path $root 'voxel.exe'
    Copy-Item -LiteralPath $Executable -Destination $game
    $relayExe = (Resolve-Path -LiteralPath 'target/release-fast/voxel-lobby.exe').Path
}
$peers = [Collections.Generic.List[object]]::new()
$hidden = [Collections.Generic.List[IO.FileInfo]]::new()
function Start-Peer($label, $binary, $save, [string[]]$arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new($binary)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardError = $true
    $info.RedirectStandardOutput = $true
    $info.Environment['VOXEL_SAVE_DIR'] = $save
    if ($Punch) {
        $info.Environment['VOXEL_STUN'] = 'off'
        $info.Environment['VOXEL_STEAM_DEDICATED'] = '0'
        $info.Environment['VOXEL_STEAM_FORCE_SDR'] = '0'
    }
    New-Item -ItemType Directory -Force -Path $save | Out-Null
    foreach ($argument in $arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($info)
    $err = [IO.FileStream]::new((Join-Path $root "$label.err"), 'Create', 'Write', 'ReadWrite', 1, $true)
    $out = [IO.FileStream]::new((Join-Path $root "$label.out"), 'Create', 'Write', 'ReadWrite', 1, $true)
    $peer = [pscustomobject]@{
        Process = $process; ErrorPath = $err.Name; ErrorFile = $err; OutputFile = $out
        ErrorCopy = $process.StandardError.BaseStream.CopyToAsync($err)
        OutputCopy = $process.StandardOutput.BaseStream.CopyToAsync($out)
    }
    $peers.Add($peer)
    $peer
}
function Wait-Log($peer, [string]$pattern, [int]$seconds = 60) {
    $deadline = [DateTime]::UtcNow.AddSeconds($seconds)
    do {
        if ($peer.Process.HasExited) {
            $null = $peer.ErrorCopy.GetAwaiter().GetResult()
        }
        $log = Get-Content -LiteralPath $peer.ErrorPath -Raw
        if ($log -match $pattern) { return $Matches }
        if ($peer.Process.HasExited) { throw "Exited while waiting for $pattern in $($peer.ErrorPath)" }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $pattern in $($peer.ErrorPath)"
}
function Send-Line($peer, [string]$line) {
    $peer.Process.StandardInput.WriteLine($line)
    $peer.Process.StandardInput.Flush()
}
function Restore-Checkpoints {
    while ($hidden.Count) {
        $file = $hidden[$hidden.Count - 1]
        Rename-Item -LiteralPath "$($file.FullName).review-hidden" -NewName $file.Name
        $hidden.RemoveAt($hidden.Count - 1)
    }
}
function Interrupt-Host($hostPeer, [object[]]$guests) {
    $hostPeer.Process.Kill()
    if (!$hostPeer.Process.WaitForExit(5000)) { throw 'Host did not terminate' }
    foreach ($peer in $guests) {
        if (!$peer.Process.HasExited) { $peer.Process.Kill() }
        if (!$peer.Process.WaitForExit(5000)) { throw 'Guest did not terminate' }
    }
}
$hostSave = Join-Path $root 'host-save'
$hostArgs = @('--multiplayer-headless','--multiplayer-open-world=RealmReview','--multiplayer-trace')
if ($Punch) {
    $hostArgs += '--multiplayer-host=punch'
    $invitePattern = 'punch offer=(\S+)'
} else {
    $hostArgs += @('--multiplayer-online=RealmReview',"--multiplayer-lobby=127.0.0.1:$Port",'--multiplayer-unlisted')
    $invitePattern = 'listed code=([A-Z0-9-]+)'
}
function Join-Peer([string]$name, [string]$suffix, [string]$code) {
    $arguments = @(
        '--multiplayer-headless',"--multiplayer-join-code=$code",
        "--multiplayer-name=$name",'--multiplayer-review-realms','--multiplayer-trace'
    )
    if (!$Punch) { $arguments += "--multiplayer-lobby=127.0.0.1:$Port" }
    $peer = Start-Peer "$name$suffix" $game (Join-Path $root "$name-save") $arguments
    if ($Punch) {
        $answer = (Wait-Log $peer 'punch answer=(\S+)')[1]
        Send-Line $hostGame $answer
        Wait-Log $peer 'punched direct UDP path' | Out-Null
    }
    $peer
}
try {
    $fixture = Start-Peer fixture $game $hostSave @('--multiplayer-headless','--multiplayer-realm-fixture')
    $coords = (Wait-Log $fixture 'surface=([a-zA-Z0-9-]+) hell=([a-zA-Z0-9-]+) gate=([^,\r\n]+),([^,\r\n]+),([^,\r\n]+)' 100)
    $surface = 'opened world=RealmReview root=[^\r\n]*[\\/]' + [regex]::Escape($coords[1]) + '(?:\r?\n|$)'
    $hellCheckpoints = Join-Path $hostSave "regions/$($coords[2])/checkpoints"
    $gate = "$($coords[3]) $($coords[4]) $($coords[5])"
    if (!$fixture.Process.WaitForExit(5000) -or $fixture.Process.ExitCode -ne 0) { throw 'Fixture failed' }
    if (!$Punch) { $relay = Start-Peer relay $relayExe (Join-Path $root 'relay-save') @('--bind',"127.0.0.1:$Port") }
    $hostGame = Start-Peer host $game $hostSave $hostArgs
    Wait-Log $hostGame $surface | Out-Null
    $code = (Wait-Log $hostGame $invitePattern)[1]
    $a = Join-Peer RealmA '' $code
    $b = Join-Peer RealmB '' $code
    $c = Join-Peer RealmC '' $code
    foreach ($peer in @($a,$b,$c)) { Wait-Log $peer 'ready=true.*players=2' | Out-Null }
    foreach ($name in @('RealmA', 'RealmB', 'RealmC')) { Send-Line $hostGame "/gamemode creative $name" }
    Send-Line $hostGame '/give RealmA stone 17'
    Send-Line $hostGame '/give RealmB stone 23'
    Send-Line $hostGame '/give RealmC stone 31'
    Wait-Log $a 'realm=Hills stone=17\b' | Out-Null
    Wait-Log $b 'realm=Hills stone=23\b' | Out-Null
    Wait-Log $c 'realm=Hills stone=31\b' | Out-Null
    foreach ($name in @('RealmA', 'RealmB', 'RealmC')) { Send-Line $hostGame "/gamemode survival $name" }
    Send-Line $hostGame "/tp RealmA $gate"
    Start-Sleep -Seconds 2
    Send-Line $a 'review-gate'
    Wait-Log $a 'realm=Nether stone=17\b' | Out-Null
    if ($b.Process.HasExited -or $c.Process.HasExited) { throw 'A surface client exited during the split' }
    Send-Line $a 'quit'
    if (!$a.Process.WaitForExit(5000)) { throw 'Guest did not disconnect' }
    $a = Join-Peer RealmA '-rejoin' $code
    Wait-Log $a 'realm=Nether stone=17\b' | Out-Null
    Interrupt-Host $hostGame @($a, $b, $c)
    # Keep the surface's named checkpoint. Temporarily hide all paths that
    # could resolve the Hell checkpoint, including the fixture's quicksave.
    $checkpoints = Get-ChildItem -LiteralPath $hellCheckpoints -Filter 'commit-*.vxw' -File
    if (!$checkpoints) { throw 'The fixture has no Hell checkpoint' }
    $slots = @('quicksave.vxw', 'quicksave.prev.vxw') | ForEach-Object {
        Get-Item -LiteralPath (Join-Path $hostSave $_) -ErrorAction SilentlyContinue
    }
    foreach ($file in (@($checkpoints) + @($slots))) {
        Rename-Item -LiteralPath $file.FullName -NewName "$($file.Name).review-hidden"
        $hidden.Add($file)
    }
    $hostGame = Start-Peer host-restart $game $hostSave $hostArgs
    Wait-Log $hostGame $surface | Out-Null
    $code = (Wait-Log $hostGame $invitePattern)[1]
    $missing = Join-Peer RealmA '-missing' $code
    Wait-Log $hostGame 'could not reopen guest realm for player .*Linked realm checkpoint missing' | Out-Null
    if (!$missing.Process.WaitForExit(10000)) { throw 'Guest joined while the saved realm was missing' }
    if ((Get-Content -LiteralPath $missing.ErrorPath -Raw) -match 'realm=Hills|synchronized player=') { throw 'Missing realm joined the wrong world' }
    Restore-Checkpoints
    $a = Join-Peer RealmA '-restart' $code
    $b = Join-Peer RealmB '-restart' $code
    $c = Join-Peer RealmC '-restart' $code
    Wait-Log $a 'realm=Nether stone=17\b' | Out-Null
    Wait-Log $b 'realm=Hills stone=23\b' | Out-Null
    Wait-Log $c 'realm=Hills stone=31\b' | Out-Null
    Send-Line $a 'review-gate'
    Wait-Log $a 'realm=Hills stone=17\b' | Out-Null
    Interrupt-Host $hostGame @($a, $b, $c)
    $hostGame = Start-Peer host-return $game $hostSave $hostArgs
    Wait-Log $hostGame $surface | Out-Null
    $code = (Wait-Log $hostGame $invitePattern)[1]
    $a = Join-Peer RealmA '-return' $code
    Wait-Log $a 'realm=Hills stone=17\b' | Out-Null
    "PASS three guests, split realms, disconnect, missing realm refusal, restart recovery, inventory, durable return. $root"
} finally {
    foreach ($peer in $peers) {
        if (!$peer.Process.HasExited) { $peer.Process.Kill(); $peer.Process.WaitForExit() }
        $null = $peer.ErrorCopy.GetAwaiter().GetResult()
        $null = $peer.OutputCopy.GetAwaiter().GetResult()
        $peer.ErrorFile.Dispose()
        $peer.OutputFile.Dispose()
    }
    Restore-Checkpoints
    "Evidence: $root"
}
