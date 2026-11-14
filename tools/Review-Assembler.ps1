param(
    [string]$Executable = "$PSScriptRoot/../target/debug/minecraft_style_voxel_prototype.exe",
    [string]$Root = "$PSScriptRoot/../artifacts/assembler-review-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [ValidateSet('Window','Network','Both')][string]$Mode = 'Both',
    [string[]]$Scales = @('1','0.5'),
    [int]$Port = 27047,
    [int]$TimeoutSeconds = 360,
    [switch]$SingleBatch,
    [switch]$Steamworks
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$source = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $Root) { throw 'Choose a fresh review directory.' }
$Root = (New-Item -ItemType Directory -Path $Root).FullName
$exe = Join-Path $Root 'candidate.exe'
Copy-Item -LiteralPath $source -Destination $exe
if ($Steamworks) {
    Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $source) 'steam_api64.dll') -Destination (Join-Path $Root 'steam_api64.dll')
}
Get-FileHash -LiteralPath $exe | ConvertTo-Json | Set-Content "$Root/executable.json"
function Info($Path) {
    $info = [Diagnostics.ProcessStartInfo]::new($Path)
    $info.WorkingDirectory = $repo
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.Environment.Remove('VOXEL_RELAY_PASSWORD') | Out-Null
    if ($SingleBatch) { $info.Environment['VOXEL_REVIEW_ASSEMBLER_BATCH'] = '1' }
    if ($Steamworks) {
        $info.Environment['VOXEL_STEAM_APP_ID'] = '480'
        $info.Environment['SteamAppId'] = '480'
        $info.Environment['VOXEL_STEAM_FORCE_SDR'] = '0'
        $info.Environment['VOXEL_STEAM_DEDICATED'] = '1'
    }
    $info
}
if ($Mode -in @('Window','Both')) {
    foreach ($scale in $Scales) {
        $run = (New-Item -ItemType Directory -Path "$Root/window-$scale").FullName
        $info = Info $exe
        $info.Environment['VOXEL_SAVE_DIR'] = Join-Path $run 'disposable-saves'
        $info.Environment['VOXEL_CAPTURE_DIR'] = $run
        $info.Environment['VOXEL_INDUSTRIAL_REVIEW'] = 'assembler'
        $info.Environment['VOXEL_REVIEW_SKIP_CLOUDS'] = '1'
        $info.Environment['VOXEL_REVIEW_SKIP_HORIZON'] = '1'
        foreach ($arg in @('--benchmark-world=superflat',"--benchmark-scale=$scale",'--benchmark-motion=static',
            '--benchmark-width=960','--benchmark-height=540','--benchmark-view-chunks=3','--benchmark-warmup=1',
            "--benchmark-duration=$TimeoutSeconds",'--benchmark-review-time=0.5','--benchmark-weather=clear','--benchmark-physics')) {
            $info.ArgumentList.Add($arg)
        }
        $p = [Diagnostics.Process]::Start($info)
        $stdout = $p.StandardOutput.ReadToEndAsync()
        $stderr = $p.StandardError.ReadToEndAsync()
        try {
            if (!$p.WaitForExit($TimeoutSeconds * 1000)) { throw "Window review timed out: $run" }
        } finally {
            if (!$p.HasExited) { $p.Kill(); $p.WaitForExit() }
            $stdout.GetAwaiter().GetResult() | Set-Content "$run/stdout.log"
            $stderr.GetAwaiter().GetResult() | Set-Content "$run/stderr.log"
            $p.ExitCode | Set-Content "$run/exit-code.txt"
        }
        $text = Get-Content "$run/stderr.log" -Raw
        if ($p.ExitCode -ne 0 -or $text -notmatch '\[assembler-review\] PASS' -or $text -match '\[assembler-review\] FAIL') {
            throw "Window review failed: $run"
        }
        if (@(Get-ChildItem -LiteralPath $run -Filter 'assembler-*.png').Count -lt 3) { throw "Missing rendered captures: $run" }
        Select-String -LiteralPath "$run/stderr.log" -SimpleMatch '[assembler-review]' | ForEach-Object { $_.Line }
    }
}
if ($Mode -eq 'Window') { Write-Output "PASS window reviews: $Root"; exit 0 }
$address = "127.0.0.1:$Port"
$children = [Collections.Generic.List[object]]::new()
$common = @('--multiplayer-headless','--multiplayer-trace','--multiplayer-review-bases','--multiplayer-review-assembler',"--multiplayer-run-seconds=$TimeoutSeconds")
if ($SingleBatch) { $common += '--multiplayer-review-assembler-batch' }
function Start-Peer($Label, $Profile, $Arguments) {
    $save = Join-Path $Root $Profile
    if (!(Test-Path -LiteralPath $save)) { New-Item -ItemType Directory -Path $save | Out-Null }
    $info = Info $exe
    $info.Environment['VOXEL_SAVE_DIR'] = $save
    foreach ($arg in ($common + $Arguments)) { $info.ArgumentList.Add($arg) }
    $p = [Diagnostics.Process]::Start($info)
    $peer = [pscustomobject]@{ Label=$Label; Process=$p; Pending=$p.StandardError.ReadLineAsync()
        Output=$p.StandardOutput.ReadToEndAsync(); Lines=[Collections.Generic.List[string]]::new()
        Ledgers=[Collections.Generic.List[object]]::new(); Ended=$false }
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
            if ($line -match '^\[assembler-review\] ledger=(.+)$') {
                $ledger = $Matches[1] | ConvertFrom-Json
                $peer.Ledgers.Add($ledger)
                if ($peer.Label -eq 'host' -and ($ledger.copper_total -ne 4096 -or $ledger.fiber_total -ne 2048)) {
                    throw "Host inventory changed: $line"
                }
            }
            if ($line -match '\[assembler-review\] FAIL') { throw $line }
        }
    }
}
function Wait-For($Peer, [string]$Pattern, [int]$Seconds = 180) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        Pump
        foreach ($line in $Peer.Lines) { if ($line -match $Pattern) { return $line } }
        if ($Peer.Process.HasExited) { throw "$($Peer.Label) exited before '$Pattern'." }
        Start-Sleep -Milliseconds 60
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$($Peer.Label) did not report '$Pattern'. See $Root."
}
function Send($Peer, $Line) {
    $Peer.Process.StandardInput.WriteLine($Line)
    $Peer.Process.StandardInput.Flush()
}
function Batch-Complete($Ledger, $Count) {
    $Ledger.destination_coils -eq $Count -and $Ledger.mode -eq 1 -and !$Ledger.enabled -and
        $Ledger.copper_total -eq 4096 -and $Ledger.fiber_total -eq 2048 -and
        $Ledger.source_copper -eq (64 - 2 * $Count) -and $Ledger.source_fiber -eq (32 - $Count) -and
        ($Ledger.input_copper + $Ledger.input_fiber + $Ledger.output_units + $Ledger.progress) -eq 0
}
function Complete($Ledger) {
    if ($SingleBatch) { return (Batch-Complete $Ledger 2) }
    $Ledger.destination_coils -eq 32 -and $Ledger.copper_total -eq 4096 -and $Ledger.fiber_total -eq 2048 -and
        ($Ledger.source_copper + $Ledger.source_fiber + $Ledger.input_copper + $Ledger.input_fiber + $Ledger.output_units + $Ledger.progress) -eq 0
}
function Production-Signature($Ledger) {
    $Ledger | Select-Object -Property * -ExcludeProperty energy | ConvertTo-Json -Compress
}
function Wait-Batch($Peer, $Count) {
    $deadline = [DateTime]::UtcNow.AddSeconds(40)
    do {
        Pump
        if ($Peer.Ledgers.Count -gt 0 -and (Batch-Complete $Peer.Ledgers[-1] $Count)) { return }
        if ($Peer.Process.HasExited) { throw "$($Peer.Label) exited before batch $Count." }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$($Peer.Label) did not settle batch $Count."
}
function Wait-Complete($Peer) {
    $deadline = [DateTime]::UtcNow.AddSeconds(100)
    do {
        Pump
        if ($Peer.Ledgers.Count -gt 0 -and (Complete $Peer.Ledgers[-1])) { return $Peer.Ledgers[-1] }
        if ($Peer.Process.HasExited) { throw "$($Peer.Label) exited before completion." }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$($Peer.Label) did not finish the requested production."
}
try {
    $reservation = [Net.Sockets.UdpClient]::new()
    try {
        $reservation.Client.ExclusiveAddressUse = $true
        $reservation.Client.Bind([Net.IPEndPoint]::new([Net.IPAddress]::Loopback, $Port))
    } finally { $reservation.Dispose() }
    $hostPeer = Start-Peer 'host' 'host-profile' @("--multiplayer-host=$address",'--multiplayer-ephemeral','--multiplayer-seed=617')
    Wait-For $hostPeer '\[assembler-review\] installed batches=32' | Out-Null
    Wait-For $hostPeer '\[multiplayer\] listening' | Out-Null
    $joinArgs = @("--multiplayer-join=$address")
    $a = Start-Peer 'a' 'a-profile' ($joinArgs + '--multiplayer-name=AssemblerA')
    $b = Start-Peer 'b' 'b-profile' ($joinArgs + '--multiplayer-name=AssemblerB')
    foreach ($peer in @($a,$b)) { Wait-For $peer 'synchronized' | Out-Null }
    if ($SingleBatch) {
        Send $a 'review-assembler-batch'
        Send $a 'review-assembler-batch'
        Send $a 'review-assembler-start'
        foreach ($peer in @($hostPeer,$a,$b)) { Wait-Batch $peer 1 }
        $hold = [DateTime]::UtcNow.AddSeconds(3)
        do {
            Pump
            foreach ($peer in @($hostPeer,$a,$b)) {
                if (!(Batch-Complete $peer.Ledgers[-1] 1)) { throw "$($peer.Label) advanced a stopped batch." }
            }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $hold)
        Send $a 'review-assembler-start'
    }
    $final = Wait-Complete $hostPeer
    foreach ($peer in @($a,$b)) {
        $null = Wait-Complete $peer
        $outputs = @($peer.Ledgers | ForEach-Object { $_.destination_coils } | Select-Object -Unique)
        if ($outputs.Count -lt $(if ($SingleBatch) { 2 } else { 3 })) { throw "$($peer.Label) did not observe multiple production updates." }
        if ((Production-Signature $peer.Ledgers[-1]) -ne (Production-Signature $final)) { throw "$($peer.Label) final ledger differs." }
    }
    Send $b 'quit'
    if (!$b.Process.WaitForExit(8000) -or $b.Process.ExitCode -ne 0) { throw 'Client B did not exit cleanly.' }
    $b2 = Start-Peer 'b-rejoin' 'b-profile' ($joinArgs + '--multiplayer-name=AssemblerB')
    Wait-For $b2 'synchronized' | Out-Null
    $rejoined = Wait-Complete $b2
    if ((Production-Signature $rejoined) -ne (Production-Signature $final)) { throw 'Reconnect final ledger differs.' }
    foreach ($peer in @($hostPeer,$a,$b2)) { if ($peer.Process.HasExited) { throw "$($peer.Label) disconnected." } }
    [pscustomobject]@{ Result='PASS'; Transport='native direct UDP on 127.0.0.1'; Batches=$(if ($SingleBatch) { 2 } else { 32 }); SingleBatch=[bool]$SingleBatch; ClientRequested=[bool]$SingleBatch; Final=$final
        SamplesA=$a.Ledgers.Count; SamplesB=$b.Ledgers.Count; ReconnectSameProfile=$true
        Limitations='Headless clients; no rendered multiplayer, WAN, server restart, or failure injection.' } |
        ConvertTo-Json -Depth 5 | Set-Content "$Root/result.json"
    Get-Content "$Root/result.json"
} finally {
        foreach ($peer in $children) {
            try { if (!$peer.Process.HasExited) { Send $peer 'quit' } } catch { Write-Warning $_ }
        }
        foreach ($peer in $children) {
            try {
                if (!$peer.Process.WaitForExit(8000)) { $peer.Process.Kill(); $peer.Process.WaitForExit() }
                $peer.Output.GetAwaiter().GetResult() | Set-Content "$Root/$($peer.Label)-stdout.log"
                while (!$peer.Ended) {
                    $line = $peer.Pending.GetAwaiter().GetResult()
                    if ($null -eq $line) { $peer.Ended = $true; break }
                    [IO.File]::AppendAllText("$Root/$($peer.Label).log", "$line`n")
                    $peer.Pending = $peer.Process.StandardError.ReadLineAsync()
                }
            } catch { Write-Warning $_ }
        }
}
Write-Output "PASS assembler acceptance: $Root"
