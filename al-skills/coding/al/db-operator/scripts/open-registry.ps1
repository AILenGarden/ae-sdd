param([switch]$NoBrowser)
$ErrorActionPreference = 'Stop'
$packageRoot = Split-Path -Parent $PSScriptRoot
$node = Get-Command node -ErrorAction SilentlyContinue
if (-not $node) { throw 'Node.js is required to run the registration UI.' }
$port = if ($env:DB_OPERATOR_UI_PORT) { [int]$env:DB_OPERATOR_UI_PORT } else { 17842 }
$uiUrl = "http://127.0.0.1:$port/"
try {
    $session = Invoke-RestMethod ($uiUrl + 'api/session') -TimeoutSec 2
    if ($session.service -ne 'db-operator' -or $session.version -ne 2) { throw 'Port is occupied by an older or different service.' }
    if (-not $NoBrowser) { Start-Process $uiUrl }
    exit 0
} catch {
    if ($session) { throw }
}
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    # RunAs requests the standard Windows consent dialog. Preserve the script path with spaces.
    $arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '"'
    if ($NoBrowser) { $arguments += ' -NoBrowser' }
    $child = Start-Process powershell.exe -Verb RunAs -WindowStyle Hidden -ArgumentList $arguments -PassThru
    # Wait for the launcher, not the long-running Node descendant.
    $child.WaitForExit()
    if ($child.ExitCode -ne 0) {
        throw ("Registration UI setup failed ({0}). Common causes: Node.js is not installed, port {1} is occupied by another program (set DB_OPERATOR_UI_PORT to change it), or the admin binary is missing. Run 'node scripts\registry-server.mjs' in an elevated PowerShell in this directory to see the detailed error." -f $child.ExitCode, $port)
    }
    exit 0
}
$env:DB_OPERATOR_HOME = Join-Path $env:ProgramData 'db-operator'
$env:DB_OPERATOR_ADMIN = Join-Path $packageRoot 'bin\service\windows-x86_64\db-operator-admin.exe'
$server = Join-Path $PSScriptRoot 'registry-server.mjs'
if (-not (Test-Path -LiteralPath $env:DB_OPERATOR_ADMIN)) { throw 'Admin binary is missing.' }
$logOut = Join-Path ([IO.Path]::GetTempPath()) ("db-operator-ui-$PID-out.log")
$logErr = Join-Path ([IO.Path]::GetTempPath()) ("db-operator-ui-$PID-err.log")
$process = Start-Process -FilePath $node.Source -ArgumentList ('"' + $server + '"') -WorkingDirectory $packageRoot -WindowStyle Hidden -RedirectStandardOutput $logOut -RedirectStandardError $logErr -PassThru
for ($attempt = 0; $attempt -lt 40; $attempt++) {
    if ($process.HasExited) {
        $detail = (Get-Content -LiteralPath $logErr -Raw -ErrorAction SilentlyContinue)
        throw "Registration server exited before becoming ready. $detail"
    }
    try {
        $session = Invoke-RestMethod ($uiUrl + 'api/session') -TimeoutSec 1
        if ($session.service -eq 'db-operator' -and $session.version -eq 2) {
            if (-not $NoBrowser) { Start-Process $uiUrl }
            exit 0
        }
    } catch { if ($attempt -eq 39) { throw 'Registration server did not become ready.' } }
    Start-Sleep -Milliseconds 250
}
throw 'Registration server did not become ready.'
