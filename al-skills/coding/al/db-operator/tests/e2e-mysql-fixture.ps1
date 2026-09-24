<#
Create (or refresh) the MySQL fixture used by tests/e2e-flow-windows.ps1: database
`appdb` with table `orders`, plus a read-only account `dbo_reader` and a DML account
`dbo_writer`. Existing rows are reset so repeated runs start from three rows.

    powershell -ExecutionPolicy Bypass -File tests/e2e-mysql-fixture.ps1 `
        -MysqlRoot <MySQL server dir> [-DataDir <temp dir>] [-Port 33306]

With -DataDir the script initializes a throwaway MySQL data directory and starts the
server itself (port 33306 by default); without it the script only seeds an already
running instance reachable as root on -Port. Nothing outside -DataDir is modified.
#>
param(
    [string]$MysqlRoot = 'C:\Program Files\MySQL\MySQL Server 8.0',
    [string]$DataDir,
    [int]$Port = 33306,
    [string]$RootUser = 'root',
    [string]$RootPassword = ''
)

$ErrorActionPreference = 'Stop'
$mysqld = Join-Path $MysqlRoot 'bin\mysqld.exe'
$mysql = Join-Path $MysqlRoot 'bin\mysql.exe'
foreach ($binary in @($mysqld, $mysql)) {
    if (-not (Test-Path -LiteralPath $binary)) { throw "Missing MySQL binary: $binary" }
}

function Invoke-MySql([string[]]$Arguments, [string]$InputText = '') {
    $stdout = [IO.Path]::GetTempFileName()
    $stderr = [IO.Path]::GetTempFileName()
    $stdinFile = $null
    # Start-Process joins an array without quoting, which merges the SQL into the previous
    # argument, so each argument is quoted here before it reaches the command line.
    $commandLine = @($Arguments | ForEach-Object { '"' + ($_ -replace '"', '\"') + '"' }) -join ' '
    if ($RootPassword) { $commandLine += ' "-p' + $RootPassword + '"' }
    $params = @{
        FilePath = $mysql; ArgumentList = $commandLine; NoNewWindow = $true; Wait = $true; PassThru = $true
        RedirectStandardOutput = $stdout; RedirectStandardError = $stderr
    }
    if ($InputText) {
        $stdinFile = [IO.Path]::GetTempFileName()
        [IO.File]::WriteAllText($stdinFile, $InputText)
        $params.RedirectStandardInput = $stdinFile
    }
    try {
        $process = Start-Process @params
        $text = (Get-Content -LiteralPath $stdout -Raw -ErrorAction SilentlyContinue) + (Get-Content -LiteralPath $stderr -Raw -ErrorAction SilentlyContinue)
        if ($process.ExitCode -ne 0) {
            throw ("mysql failed ({0}): {1}" -f $process.ExitCode, $text)
        }
        return $text
    }
    finally {
        Remove-Item -LiteralPath $stdout, $stderr -Force -ErrorAction SilentlyContinue
        if ($stdinFile) { Remove-Item -LiteralPath $stdinFile -Force -ErrorAction SilentlyContinue }
    }
}

$startedProcess = $null
if ($DataDir) {
    if (Test-Path -LiteralPath $DataDir) { Remove-Item -LiteralPath $DataDir -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $DataDir, (Join-Path $DataDir 'log') | Out-Null
    & $mysqld --no-defaults --initialize-insecure --basedir="$MysqlRoot" --datadir="$DataDir" --log-error="$(Join-Path $DataDir 'log\init.log')"
    if ($LASTEXITCODE -ne 0) { throw "MySQL initialization failed ($LASTEXITCODE)" }
    $startedProcess = Start-Process -FilePath $mysqld -PassThru -WindowStyle Hidden -ArgumentList @(
        '--no-defaults', "--basedir=$MysqlRoot", "--datadir=$DataDir", "--port=$Port", '--bind-address=127.0.0.1',
        '--mysqlx=0', "--log-error=$(Join-Path $DataDir 'log\server.log')", "--pid-file=$(Join-Path $DataDir 'mysqld.pid')"
    )
    $ready = $false
    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        Start-Sleep -Milliseconds 500
        try { Invoke-MySql @('-h', '127.0.0.1', "-P$Port", "-u$RootUser", '--connect-timeout=2', '-e', 'SELECT 1') | Out-Null; $ready = $true; break } catch { }
        if ($startedProcess.HasExited) { break }
    }    if (-not $ready) { throw "MySQL did not become ready; see $(Join-Path $DataDir 'log\server.log')" }
}

$fixture = @"
CREATE DATABASE IF NOT EXISTS appdb;
CREATE TABLE IF NOT EXISTS appdb.orders (id INT PRIMARY KEY, customer VARCHAR(64), amount DECIMAL(10,2));
DELETE FROM appdb.orders;
INSERT INTO appdb.orders VALUES (1,'alice',10.50),(2,'bob',20.00),(3,'carol',7.25);
DROP USER IF EXISTS 'dbo_reader'@'%';
DROP USER IF EXISTS 'dbo_writer'@'%';
CREATE USER 'dbo_reader'@'%' IDENTIFIED WITH mysql_native_password BY 'reader-secret-1';
CREATE USER 'dbo_writer'@'%' IDENTIFIED WITH mysql_native_password BY 'writer-secret-1';
GRANT SELECT ON appdb.* TO 'dbo_reader'@'%';
GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, ALTER, DROP, INDEX ON appdb.* TO 'dbo_writer'@'%';
FLUSH PRIVILEGES;
"@
Invoke-MySql @('-h', '127.0.0.1', "-P$Port", "-u$RootUser", '--connect-timeout=5', '-e', $fixture) | Out-Null
$rows = (Invoke-MySql @('-h', '127.0.0.1', "-P$Port", "-u$RootUser", '-N', '-e', 'SELECT COUNT(*) FROM appdb.orders')).Trim()
Write-Output "MySQL fixture ready on 127.0.0.1:$Port (appdb.orders rows=$rows)"
if ($startedProcess) {
    Write-Output "Server PID $($startedProcess.Id); data directory: $DataDir"
    Write-Output 'Stop it with: Stop-Process -Id <pid>'
}
