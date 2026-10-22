# Native review reads must see a complete command. Atomic replacement also
# avoids Set-Content's exclusive-open race against Rust's file reader on Windows.
function Write-MeridianCommand([string]$Root,[string]$Value) {
    $pending=Join-Path $Root 'command.next'
    [IO.File]::WriteAllText($pending,$Value)
    for($attempt=0;$attempt -lt 50;$attempt++) {
        # File.Move's overwrite overload is .NET Core only; Windows PowerShell 5.1
        # gets the same atomic swap from File.Replace once the target exists.
        $target=Join-Path $Root 'command.txt'
        try {if(Test-Path -LiteralPath $target){[IO.File]::Replace($pending,$target,[NullString]::Value)}else{[IO.File]::Move($pending,$target)};return}
        catch [IO.IOException] {if($attempt -eq 49){throw};Start-Sleep -Milliseconds 10}
    }
}
