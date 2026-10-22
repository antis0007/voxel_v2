# Native review reads must see a complete command. Atomic replacement also
# avoids Set-Content's exclusive-open race against Rust's file reader on Windows.
function Write-MeridianCommand([string]$Root,[string]$Value) {
    $pending=Join-Path $Root 'command.next'
    [IO.File]::WriteAllText($pending,$Value)
    for($attempt=0;$attempt -lt 50;$attempt++) {
        try {[IO.File]::Move($pending,(Join-Path $Root 'command.txt'),$true);return}
        catch [IO.IOException] {if($attempt -eq 49){throw};Start-Sleep -Milliseconds 10}
    }
}
