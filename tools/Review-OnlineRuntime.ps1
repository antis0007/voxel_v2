param(
    [string]$Executable = 'target/online-responsive.exe',
    [string]$Label = "review-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [int]$Seconds = 50,
    [int]$Port = 27129,
    [switch]$Rendered,
    [switch]$Gameplay,
    [switch]$RenderedOnly,
    [ValidateRange(1,2)][int]$RenderedClients = 2
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $repo
$root = Join-Path $repo "target/online-runtime/$Label"
if (Test-Path -LiteralPath $root) { throw "Evidence already exists: $root" }
New-Item -ItemType Directory -Path $root | Out-Null
$deadline = (Get-Date).AddMinutes(5)
while (-not (Test-Path -LiteralPath $Executable)) {
    if ((Get-Date) -gt $deadline) { throw 'Candidate build missing' }
    Start-Sleep -Seconds 1
}
$exe = (Resolve-Path -LiteralPath $Executable).Path
$env:VOXEL_NETWORK_PROFILE = '1'
$env:VOXEL_GAMEPLAY_TRACE = '1'
$processes = [Collections.Generic.List[Diagnostics.Process]]::new()
$streams = [Collections.Generic.List[object]]::new()
function Pump {
    foreach ($stream in $streams) {
        if ($stream.Task.IsFaulted) { $stream.Task.GetAwaiter().GetResult() }
    }
}
function Launch([string]$Name, [string]$Binary, [string[]]$GameArgs) {
    $env:VOXEL_SAVE_DIR = Join-Path $root "$Name-save"
    if ($Name -eq 'host') {
        New-Item -ItemType Directory -Path $env:VOXEL_SAVE_DIR -Force | Out-Null
        Set-Content -LiteralPath "$env:VOXEL_SAVE_DIR/server.properties" -Value "spawn-protection=0"
    }
    $info = [Diagnostics.ProcessStartInfo]::new($Binary)
    $info.WorkingDirectory = $repo
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in $GameArgs) { $info.ArgumentList.Add($arg) }
    $p = [Diagnostics.Process]::Start($info)
    foreach ($pair in @(@('out', $p.StandardOutput), @('err', $p.StandardError))) {
        $path = "$root/$Name.$($pair[0])"
        # Drain on .NET's I/O threads, independently of orchestration polling.
        # A line-at-a-time reader polled every 500 ms can fill the OS pipe and
        # manufacture half-second game stalls when renderer tracing is enabled.
        $file = [IO.FileStream]::new($path, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite, 1, $true)
        $streams.Add([pscustomobject]@{File=$file;Task=$pair[1].BaseStream.CopyToAsync($file)})
    }
    $processes.Add($p)
    return $p
}
try {
    $relay = Launch 'relay' "$repo/target/release-fast/voxel-lobby.exe" @('--bind',"127.0.0.1:$Port")
    $hostGame = Launch 'host' $exe @('--multiplayer-online=RuntimeReview',"--multiplayer-lobby=127.0.0.1:$Port",'--multiplayer-headless','--multiplayer-ephemeral','--multiplayer-unlisted','--multiplayer-trace','--multiplayer-run-seconds=240')
    $deadline = (Get-Date).AddSeconds(90)
    $code = $null
    while (-not $code) {
        Pump
        if ($hostGame.HasExited) { throw "Host exited: $(Get-Content $root/host.err -Tail 8)" }
        $log = Get-Content "$root/host.err" -Raw -ErrorAction SilentlyContinue
        if ($log -match 'listed code=([A-Z0-9-]+)') { $code = $Matches[1]; break }
        if ((Get-Date) -gt $deadline) { throw 'Host did not register within 90 seconds' }
        Start-Sleep -Milliseconds 250
    }
    Write-Output "Session $code registered; starting two clients"
    if (-not $RenderedOnly) {
    $common = @("--multiplayer-join-code=$code", "--multiplayer-lobby=127.0.0.1:$Port", '--multiplayer-headless','--multiplayer-trace',"--multiplayer-run-seconds=$Seconds")
    $a = Launch 'walker' $exe ($common + @('--multiplayer-name=ReviewWalker','--multiplayer-profile=11111111111111111111111111111111','--multiplayer-bot-walk'))
    $b = Launch 'worker' $exe ($common + @('--multiplayer-name=ReviewWorker','--multiplayer-profile=22222222222222222222222222222222','--multiplayer-bot-work'))
    $deadline = (Get-Date).AddSeconds($Seconds + 120)
    while (-not ($a.HasExited -and $b.HasExited)) {
        Pump
        if ((Get-Date) -gt $deadline) { throw 'Clients exceeded runtime deadline' }
        Start-Sleep -Milliseconds 500
    }
    Write-Output "Client exits: walker=$($a.ExitCode) worker=$($b.ExitCode)"
    $rejoin = Launch 'rejoin' $exe @("--multiplayer-join-code=$code", "--multiplayer-lobby=127.0.0.1:$Port", '--multiplayer-headless','--multiplayer-trace','--multiplayer-run-seconds=20','--multiplayer-name=ReviewWalker','--multiplayer-profile=11111111111111111111111111111111','--multiplayer-bot-walk')
    $deadline = (Get-Date).AddSeconds(90)
    while (-not $rejoin.HasExited) {
        Pump
        if ((Get-Date) -gt $deadline) { throw 'Rejoin exceeded runtime deadline' }
        Start-Sleep -Milliseconds 100
    }
    Pump
    Write-Output "Rejoin exit: $($rejoin.ExitCode)"
    foreach ($name in @('walker','worker','rejoin')) {
        $log = Get-Content "$root/$name.err" -Raw
        if ($log -notmatch 'ready=true' -or $log -match 'panicked|multiplayer stopped') { throw "$name failed synchronization" }
    }
    if ($a.ExitCode -ne 0 -or $b.ExitCode -ne 0 -or $rejoin.ExitCode -ne 0) { throw 'Headless clients failed' }
    if ((Get-Content "$root/walker.err" -Raw) -notmatch 'players=1') { throw 'Clients did not see each other' }
    if ((Get-Content "$root/host.err" -Raw) -notmatch 'command_ack=2') { throw 'Host did not acknowledge crafting commands' }
    }
    if ($Rendered) {
        if (-not $Gameplay) { $movingPeer = Launch 'moving-peer' $exe @("--multiplayer-join-code=$code", "--multiplayer-lobby=127.0.0.1:$Port", '--multiplayer-headless', '--multiplayer-trace', '--multiplayer-run-seconds=120', '--multiplayer-name=MovingPeer', '--multiplayer-profile=55555555555555555555555555555555', '--multiplayer-bot-walk') }
        $env:VOXEL_RENDER_TIMING = '1'
        $env:VOXEL_BENCHMARK_BARRIER = if ($Gameplay -and $RenderedClients -eq 2) { "$root/clients-ready" } else { $null }
        $env:VOXEL_CAPTURE_DIR = "$root/rendered-capture"
        $gameplayArgs = @()
        if ($Gameplay) { $gameplayArgs = @('--multiplayer-rendered-duel') }
        $env:VOXEL_REVIEW_WINDOW_SIDE = 'left'
        $renderedGame = Launch 'rendered' $exe ($gameplayArgs + @("--multiplayer-join-code=$code", "--multiplayer-lobby=127.0.0.1:$Port", '--multiplayer-name=ReviewRendered', '--multiplayer-profile=33333333333333333333333333333333', '--benchmark-width=960','--benchmark-height=540','--benchmark-view-chunks=4','--benchmark-warmup=10','--benchmark-duration=20','--benchmark-capture-frames','--benchmark-capture-hud'))
        $env:VOXEL_CAPTURE_DIR = "$root/rendered-friend-capture"
        $env:VOXEL_REVIEW_WINDOW_SIDE = 'right'
        $renderedFriend = $null
        if ($RenderedClients -eq 2) {
            $renderedFriend = Launch 'rendered-friend' $exe ($gameplayArgs + @("--multiplayer-join-code=$code", "--multiplayer-lobby=127.0.0.1:$Port", '--multiplayer-name=ReviewFriend', '--multiplayer-profile=44444444444444444444444444444444', '--benchmark-width=960','--benchmark-height=540','--benchmark-view-chunks=4','--benchmark-warmup=10','--benchmark-duration=20','--benchmark-capture-frames','--benchmark-capture-hud'))
        }
        $deadline = (Get-Date).AddSeconds(150)
        while (-not ($renderedGame.HasExited -and ($null -eq $renderedFriend -or $renderedFriend.HasExited))) {
            Pump
            if ($env:VOXEL_BENCHMARK_BARRIER -and -not (Test-Path -LiteralPath $env:VOXEL_BENCHMARK_BARRIER)) {
                $firstReady = (Get-Content "$root/rendered.err" -Raw) -match 'world_ready'
                $secondReady = (Get-Content "$root/rendered-friend.err" -Raw) -match 'world_ready'
                if ($firstReady -and $secondReady) {
                    Set-Content -LiteralPath $env:VOXEL_BENCHMARK_BARRIER -Value 'Both rendered clients ready'
                    Write-Output 'Both clients ready; starting shared warmup'
                }
            }
            if ((Get-Date) -gt $deadline) { throw 'Rendered clients exceeded runtime deadline' }
            Start-Sleep -Milliseconds 500
        }
        Pump
        $renderedNames = @('rendered')
        if ($renderedFriend) { $renderedNames += 'rendered-friend' }
        foreach ($name in $renderedNames) {
            $log = Get-Content "$root/$name.err" -Raw
            if ($log -notmatch 'world_ready' -or $log -match 'multiplayer stopped|panicked') { throw "$name failed runtime readiness" }
        }
        if ($renderedGame.ExitCode -ne 0 -or ($renderedFriend -and $renderedFriend.ExitCode -ne 0)) { throw 'Rendered clients failed' }
        if ($Gameplay) {
            foreach ($name in $renderedNames) {
                $log = Get-Content "$root/$name.err" -Raw
                if ($log -notmatch '\[rendered-duel\].*attack=true moving=true') { throw "$name did not exercise movement and attacks" }
                $positions = [regex]::Matches($log, '\[rendered-duel\].*pos=(.*?) health=') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
                if ($positions.Count -lt 5) { throw "$name did not move through the world" }
            }
            $victims = [regex]::Matches((Get-Content "$root/host.err" -Raw), '\[pvp\] target=(\d+) damage=') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique
            if ($victims.Count -lt $RenderedClients) { throw 'Host did not confirm combat damage to both players' }
        }
        if (-not $Gameplay -and (Get-Content "$root/moving-peer.err" -Raw) -notmatch 'ready=true') { throw 'Moving peer failed synchronization' }
        Write-Output "Rendered exit: $($renderedGame.ExitCode)"
    }
    [ordered]@{ executable=$exe; sha256=(Get-FileHash $exe -Algorithm SHA256).Hash; walker=$a.ExitCode; worker=$b.ExitCode; rejoin=$rejoin.ExitCode; rendered=if($Rendered){$renderedGame.ExitCode}else{$null}; rendered_friend=if($Rendered){$renderedFriend.ExitCode}else{$null}; evidence=$root } | ConvertTo-Json | Set-Content "$root/result.json"
    Get-Content "$root/result.json"
} finally {
    # Only stop the disposable processes created by this run.
    foreach ($p in $processes) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -ErrorAction SilentlyContinue } }
    Pump
    foreach ($stream in $streams) { if ($stream.Task.Wait(5000)) { $stream.File.Dispose() } }
}
