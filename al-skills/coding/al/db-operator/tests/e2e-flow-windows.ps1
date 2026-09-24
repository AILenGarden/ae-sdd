<#
End-to-end flow check for a Windows release package, run against a real MySQL server.

    powershell -ExecutionPolicy Bypass -File tests/e2e-flow-windows.ps1 `
        -Package <extracted package directory> [-MysqlPort 33306]

It needs a reachable MySQL with database `appdb` (a table `orders`), a read-only
account `dbo_reader` and a DML account `dbo_writer`; tests/e2e-mysql-fixture.ps1
creates both. It covers the whole chain: package integrity, client install, admin CLI
registration, capability issuance, daemon queries and write levels, the management UI
over HTTP, deletion cascades, and daemon-down behaviour. No changes are made to the
host system: the service is never installed and everything runs in a temp directory.
#>
param(
    [Parameter(Mandatory = $true)] [string]$Package,
    [int]$MysqlPort = 33306
)

$ErrorActionPreference = 'Stop'
$script:results = [Collections.Generic.List[object]]::new()
function Check([string]$Name, [bool]$Ok, [string]$Detail = '') {
    $script:results.Add([pscustomobject]@{ name = $Name; ok = $Ok; detail = $Detail })
    $tag = if ($Ok) { 'PASS' } else { 'FAIL' }
    Write-Output ("{0}  {1}{2}" -f $tag, $Name, $(if ($Detail) { '  | ' + $Detail } else { '' }))
}

# Native tools write progress and errors to stderr; with ErrorActionPreference = Stop
# PowerShell would turn that into a terminating error, so every call goes through here.
function Invoke-Native([string]$File, [string[]]$Arguments, [string]$StdinText = '') {
    $stdoutFile = [IO.Path]::GetTempFileName()
    $Arguments = @($Arguments | ForEach-Object { if ($_ -is [string]) { $_ } else { $_ } })
    $stderrFile = [IO.Path]::GetTempFileName()
    $stdinFile = $null
    $params = @{
        FilePath = $File; ArgumentList = $Arguments; NoNewWindow = $true; Wait = $true; PassThru = $true
        RedirectStandardOutput = $stdoutFile; RedirectStandardError = $stderrFile
    }
    if ($StdinText) {
        $stdinFile = [IO.Path]::GetTempFileName()
        [IO.File]::WriteAllText($stdinFile, $StdinText)
        $params.RedirectStandardInput = $stdinFile
    }
    try {
        $process = Start-Process @params
        $text = ((Get-Content -LiteralPath $stdoutFile -Raw -ErrorAction SilentlyContinue) + (Get-Content -LiteralPath $stderrFile -Raw -ErrorAction SilentlyContinue))
        if ($null -eq $text) { $text = '' }
        return [pscustomobject]@{ exit = $process.ExitCode; text = $text.Trim() }
    }
    finally {
        Remove-Item -LiteralPath $stdoutFile, $stderrFile -Force -ErrorAction SilentlyContinue
        if ($stdinFile) { Remove-Item -LiteralPath $stdinFile -Force -ErrorAction SilentlyContinue }
    }
}

$work = Join-Path ([IO.Path]::GetTempPath()) ('dbop-flow-' + [guid]::NewGuid().ToString('N'))
$privateDir = Join-Path $work 'private'
$agentDir = Join-Path $work 'agent'
$uiDir = Join-Path $work 'ui'
$stderrPath = Join-Path $work 'daemon.stderr.log'
New-Item -ItemType Directory -Force -Path $privateDir, $agentDir, $uiDir | Out-Null

$identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$sid = ([Security.Principal.NTAccount]$identity).Translate([Security.Principal.SecurityIdentifier]).Value
& icacls $privateDir /inheritance:r /grant:r 'SYSTEM:(OI)(CI)F' 'Administrators:(OI)(CI)F' "$identity`:(OI)(CI)F" | Out-Null

$admin = Join-Path $Package 'bin\service\windows-x86_64\db-operator-admin.exe'
$daemon = Join-Path $Package 'bin\service\windows-x86_64\db-operator-daemon.exe'
$client = Join-Path $Package 'bin\windows-x86_64\db-operator.exe'
$uiServer = Join-Path $Package 'scripts\registry-server.mjs'
$pipeName = 'db-operator-v2-flow-' + [guid]::NewGuid().ToString('N')
$endpoint = '\\.\pipe\' + $pipeName
$pipeSddl = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;$sid)"

$oldHome = $env:DB_OPERATOR_HOME
$oldClient = $env:DB_OPERATOR_CLIENT_CONFIG
$env:DB_OPERATOR_HOME = $privateDir
$script:daemonProcess = $null
$script:uiProcess = $null
$uiPort = 27910
$clientConfig = Join-Path $agentDir 'client.json'

function Invoke-DbOperator([string]$ConfigPath, [string[]]$Arguments) {
    $env:DB_OPERATOR_CLIENT_CONFIG = $ConfigPath
    return Invoke-Native $client $Arguments
}

# PowerShell drops embedded quotes when it builds a native command line, so SQL is
# handed over through the process API instead: same argv, quotes intact.
function Get-FirstCell($Result, [string]$Column) {
    # The daemon returns positional rows: read the column names to index them.
    $parsed = Read-Json $Result
    if ($null -eq $parsed -or $null -eq $parsed.rows) { return $null }
    $rowList = @($parsed.rows)
    if ($rowList.Count -eq 0) { return $null }
    $firstRow = @($rowList[0])
    if ($firstRow.Count -eq 0) { return $null }
    $colList = @($parsed.columns)
    $index = [Array]::IndexOf($colList, $Column)
    if ($index -lt 0) { $index = 0 }
    return $firstRow[$index]
}

function Read-Json($Result) {
    try { return ($Result.text | ConvertFrom-Json) } catch { return $null }
}

function Invoke-Query([string]$ConfigPath, [string]$Sql) {
    $env:DB_OPERATOR_CLIENT_CONFIG = $ConfigPath
    $stdoutFile = [IO.Path]::GetTempFileName()
    $stderrFile = [IO.Path]::GetTempFileName()
    try {
        # Start-Process joins an array without quoting, so the SQL travels inside one
        # pre-quoted command-line string: the client must receive it as a single argv entry.
        $quotedSql = '"' + $Sql.Replace('"', '\"') + '"'
        $process = Start-Process -FilePath $client -ArgumentList ('query ' + $quotedSql) -NoNewWindow -Wait -PassThru `
            -RedirectStandardOutput $stdoutFile -RedirectStandardError $stderrFile
        $text = ((Get-Content -LiteralPath $stdoutFile -Raw -ErrorAction SilentlyContinue) + (Get-Content -LiteralPath $stderrFile -Raw -ErrorAction SilentlyContinue))
        if ($null -eq $text) { $text = '' }
        return [pscustomobject]@{ exit = $process.ExitCode; text = $text.Trim() }
    }
    finally { Remove-Item -LiteralPath $stdoutFile, $stderrFile -Force -ErrorAction SilentlyContinue }
}

function Start-Daemon([string]$ConfigPath) {
    $script:daemonProcess = Start-Process -FilePath $daemon `
        -ArgumentList @('--home', $privateDir, '--endpoint', $endpoint, '--pipe-sddl', $pipeSddl) `
        -WindowStyle Hidden -RedirectStandardError $stderrPath -PassThru
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 100
        $probe = Invoke-DbOperator $ConfigPath 'status'
        if ($probe.exit -eq 0) { return $true }
        if ($script:daemonProcess.HasExited) { return $false }
    }
    return $false
}

function Stop-Daemon {
    if ($null -ne $script:daemonProcess -and -not $script:daemonProcess.HasExited) {
        Stop-Process -Id $script:daemonProcess.Id -Force -ErrorAction SilentlyContinue
        $script:daemonProcess.WaitForExit(5000) | Out-Null
    }
    $script:daemonProcess = $null
}

try {
    # ---------- Phase A: release package and client install ----------
    $versions = @((Invoke-Native $client @('--version')).text, (Invoke-Native $admin @('--version')).text, (Invoke-Native $daemon @('--version')).text)
    Check 'A1 three binaries report 3.0.0' (($versions -join '|') -eq 'db-operator 3.0.0|db-operator-admin 3.0.0|db-operator-daemon 3.0.0') ($versions -join ' | ')

    $manifest = Get-Content -LiteralPath (Join-Path $Package 'release-manifest.json') -Raw | ConvertFrom-Json
    $badHash = @()
    foreach ($entry in $manifest.files) {
        $file = Join-Path $Package ($entry.path -replace '/', '\')
        if (-not (Test-Path -LiteralPath $file)) { $badHash += "missing:$($entry.path)"; continue }
        $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($hash -ne $entry.sha256) { $badHash += "mismatch:$($entry.path)" }
    }
    Check ("A2 manifest SHA-256 verified ({0} files)" -f $manifest.files.Count) ($badHash.Count -eq 0) ($badHash -join ',')

    $zipPath = "$Package.zip"
    $zipExpected = (Get-Content -LiteralPath "$zipPath.sha256" -Raw).Split(' ')[0].Trim().ToLowerInvariant()
    $zipActual = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Check 'A3 archive .sha256 matches the file' ($zipExpected -eq $zipActual)

    $install = Invoke-Native 'powershell' @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $Package 'scripts\install.ps1'))
    Check 'A4 install.ps1 installs the agent client' ($install.exit -eq 0 -and $install.text -match 'Installed query-only db-operator client') ($install.text -replace '\s+', ' ')
    Check 'A5 install.ps1 states it does not issue client.json' ($install.text -match 'not configured')

    $helpText = (Invoke-Native $client @('--help')).text
    Check 'A6 client CLI hides admin commands' (($helpText -match '(?ms)^\s+query\b') -and ($helpText -match '(?ms)^\s+status\b') -and ($helpText -match '(?ms)^\s+schema\b') -and ($helpText -notmatch '(?ms)^\s+register\b') -and ($helpText -notmatch '(?ms)^\s+daemon\b'))

    # ---------- Phase B: admin CLI registration against the real database ----------
    $readerArgs = @(
        'register', '--name', 'shop-read', '--engine', 'mysql', '--host', '127.0.0.1', '--port', "$MysqlPort",
        '--database', 'appdb', '--username', 'dbo_reader', '--ssl-mode', 'disable',
        '--environment', 'e2e', '--connect-timeout', '5', '--statement-timeout', '15',
        '--write-level', 'none', '--password-stdin', '--test'
    )
    $readerOut = Invoke-Native $admin $readerArgs 'reader-secret-1'
    Check 'B1 register read-only connection (real connection test passes)' ($readerOut.exit -eq 0 -and $readerOut.text -match '"registered":\s*true') ($readerOut.text -replace '\s+', ' ')

    $writerArgs = @(
        'register', '--name', 'shop-write', '--engine', 'mysql', '--host', '127.0.0.1', '--port', "$MysqlPort",
        '--database', 'appdb', '--username', 'dbo_writer', '--ssl-mode', 'disable',
        '--environment', 'e2e', '--connect-timeout', '5', '--statement-timeout', '15',
        '--write-level', 'dml', '--password-stdin', '--test'
    )
    $writerOut = Invoke-Native $admin $writerArgs 'writer-secret-1'
    Check 'B2 register DML connection (real connection test passes)' ($writerOut.exit -eq 0 -and $writerOut.text -match '"write_level":\s*"dml"') ($writerOut.text -replace '\s+', ' ')

    # --test must actually reach the database: a wrong password has to fail the registration.
    $wrongArgs = @(
        'register', '--name', 'shop-bad', '--engine', 'mysql', '--host', '127.0.0.1', '--port', "$MysqlPort",
        '--database', 'appdb', '--username', 'dbo_reader', '--ssl-mode', 'disable', '--password-stdin', '--test'
    )
    $wrongOut = Invoke-Native $admin $wrongArgs 'wrong-password-xyz'
    # A readable error keeps whole words ("Access denied") and masks the account name;
    # the artifact this guards against is a redaction placeholder spliced into a word.
    Check 'B3 --test rejects a wrong password with a readable error' ($wrongOut.exit -ne 0 -and $wrongOut.text -match 'Access denied' -and $wrongOut.text -notmatch 'dbo_reader' -and $wrongOut.text -notmatch '\w<redacted>\w') ($wrongOut.text -replace '\s+', ' ')

    $reachArgs = @(
        'register', '--name', 'shop-unreachable', '--engine', 'mysql', '--host', '127.0.0.1', '--port', '33399',
        '--database', 'appdb', '--username', 'dbo_reader', '--ssl-mode', 'disable', '--connect-timeout', '3',
        '--statement-timeout', '5', '--password-stdin', '--test'
    )
    $reachOut = Invoke-Native $admin $reachArgs 'reader-secret-1'
    Check 'B4 unreachable host keeps the error text readable' ($reachOut.exit -ne 0 -and $reachOut.text -match 'timed out|timeout|refused' -and $reachOut.text -notmatch 'o<redacted>') ($reachOut.text -replace '\s+', ' ')

    $listJson = Read-Json (Invoke-Native $admin @('list', '--json'))
    Check 'B5 registry holds exactly the two good connections' ($listJson.connections.Count -eq 2 -and (($listJson.connections | ForEach-Object { $_.name }) -join ',') -eq 'shop-read,shop-write') (($listJson.connections | ForEach-Object { $_.name }) -join ',')

    $regOnDisk = Get-Content -LiteralPath (Join-Path $privateDir 'connections.json') -Raw
    Check 'B6 registry file contains no plaintext password' (-not $regOnDisk.Contains('reader-secret-1') -and -not $regOnDisk.Contains('writer-secret-1'))

    # ---------- Phase C: issuance and real queries through the daemon ----------
    $issue1 = Invoke-Native $admin @('client', 'issue', '--connection', 'shop-read', '--endpoint', $endpoint, '--output', $clientConfig)
    Check 'C1 issue read-only capability and write client.json' ($issue1.exit -eq 0 -and (Test-Path -LiteralPath $clientConfig)) ($issue1.text -replace '\s+', ' ')
    $config = Get-Content -LiteralPath $clientConfig -Raw | ConvertFrom-Json
    $capRecord = Get-Content -LiteralPath (Join-Path $privateDir 'capability.json') -Raw | ConvertFrom-Json
    Check 'C2 client.json carries no host/user metadata and the digest stays private' (
        (($config.PSObject.Properties.Name) -join ',') -eq 'endpoint,capability,connection,write_level' -and
        $capRecord.digest.Length -eq 64 -and -not $capRecord.digest.Contains($config.capability)
    ) (($config.PSObject.Properties.Name) -join ',')

    if (-not (Start-Daemon $clientConfig)) { throw ('daemon did not become ready: ' + (Get-Content -LiteralPath $stderrPath -Raw -ErrorAction SilentlyContinue)) }
    Check 'C3 daemon starts and answers status' $true

    $status = Read-Json (Invoke-DbOperator $clientConfig 'status')
    Check 'C4 status reports the bound connection and engine' ($status.connection -eq 'shop-read' -and $status.engine -eq 'mysql') ($status | ConvertTo-Json -Compress)

    $q1 = Invoke-Query $clientConfig 'SELECT id, customer, amount FROM appdb.orders ORDER BY id'
    $rows = Read-Json $q1
    $parsed = $null -ne $rows
    Check 'C5 real SELECT returns rows' ($q1.exit -eq 0 -and $parsed -and $rows.row_count -eq 3 -and $rows.rows.Count -eq 3) ('exit=' + $q1.exit + ' ' + ($q1.text -replace '\s+', ' '))

    $q2 = Invoke-Query $clientConfig "INSERT INTO appdb.orders VALUES (99, 'mallory', 1.00)"
    Check 'C6 read-only connection rejects INSERT' ($q2.exit -ne 0) ($q2.text -replace '\s+', ' ')

    $q3 = Invoke-Query $clientConfig 'DROP TABLE appdb.orders'
    Check 'C7 read-only connection rejects DROP TABLE' ($q3.exit -ne 0) ($q3.text -replace '\s+', ' ')

    $q4 = Invoke-Query $clientConfig 'SELECT 1; SELECT 2'
    Check 'C8 multi-statement input rejected' ($q4.exit -ne 0) ($q4.text -replace '\s+', ' ')

    $q5 = Invoke-Query $clientConfig 'SELECT @@version'
    Check 'C9 session variable query rejected' ($q5.exit -ne 0) ($q5.text -replace '\s+', ' ')

    $q6 = Invoke-Query $clientConfig 'SELECT * FROM appdb.no_such_table'
    Check 'C10 database error surfaces without credentials' ($q6.exit -ne 0 -and $q6.text -match 'no_such_table' -and $q6.text -notmatch 'reader-secret-1' -and $q6.text -notmatch 'dbo_reader') ($q6.text -replace '\s+', ' ')

    $q7 = Invoke-Query $clientConfig "SELECT id FROM appdb.orders WHERE customer = 'alice'"
    Check 'C11 filtered quoted query works' ($q7.exit -eq 0 -and ((Read-Json $q7).row_count) -eq 1) ($q7.text -replace '\s+', ' ')

    $fake = Join-Path $agentDir 'fake.json'
    Set-Content -LiteralPath $fake -Value ($config | ConvertTo-Json -Compress).Replace($config.capability, 'not-a-real-capability') -Encoding ascii
    $q8 = Invoke-DbOperator $fake 'status'
    Check 'C12 forged capability rejected' ($q8.exit -ne 0 -and $q8.text -match 'AUTHENTICATION|authentication') ($q8.text -replace '\s+', ' ')

    $staleAlone = Join-Path $agentDir 'missing.json'
    $q9 = Invoke-DbOperator $staleAlone 'status'
    Check 'C13 missing client.json reported clearly' ($q9.exit -ne 0 -and $q9.text -match 'client configuration|No such file|cannot read') ($q9.text -replace '\s+', ' ')

    # ---------- Phase D: schema cache ----------
    $schemaBefore = Invoke-DbOperator $clientConfig 'schema get'
    Check 'D1 schema get readable before refresh' ($schemaBefore.exit -eq 0) ($schemaBefore.text -replace '\s+', ' ')

    Invoke-Native $admin @('use', 'shop-read') | Out-Null
    $refresh = Invoke-Native $admin @('schema', 'refresh')
    Check 'D2 schema refresh against the real database' ($refresh.exit -eq 0 -and $refresh.text -match '"refreshed":\s*true') ($refresh.text -replace '\s+', ' ')
    $schemaAfter = Invoke-DbOperator $clientConfig 'schema get'
    $cacheFlat = $schemaAfter.text -replace '\s+', ' '
    Check 'D3 cache lists the orders columns' ($schemaAfter.exit -eq 0 -and $cacheFlat -match 'orders\.id' -and $cacheFlat -match 'orders\.amount') ($cacheFlat.Substring(0, [Math]::Min(150, $cacheFlat.Length)))
    Check 'D4 cached metadata carries type and position' ($cacheFlat -match '"data_type": "int"' -and $cacheFlat -notmatch '"data_type": null' -and $cacheFlat -match '"ordinal_position": 1') ($cacheFlat.Substring(0, [Math]::Min(150, $cacheFlat.Length)))

    # ---------- Phase E: write level enforcement (single-slot reissue) ----------
    $writerConfig = Join-Path $agentDir 'writer.json'
    # The admin CLI takes the write level explicitly; the UI mirrors the registered one.
    $writerIssue = Invoke-Native $admin @('client', 'issue', '--connection', 'shop-write', '--endpoint', $endpoint, '--output', $writerConfig, '--write-level', 'dml')
    Check 'E0 DML connection gets a capability carrying dml' ($writerIssue.exit -eq 0) ($writerIssue.text -replace '\s+', ' ')
    Stop-Daemon
    if (-not (Start-Daemon $writerConfig)) { throw 'daemon did not become ready with the writer capability' }
    $stale = Invoke-DbOperator $clientConfig 'status'
    Check 'E1 replaced read-only capability stops working' ($stale.exit -ne 0) ($stale.text -replace '\s+', ' ')

    $w1 = Invoke-Query $writerConfig "INSERT INTO appdb.orders VALUES (99, 'mallory', 1.00)"
    Check 'E2 DML connection can INSERT' ($w1.exit -eq 0) ($w1.text -replace '\s+', ' ')

    $w2 = Invoke-Query $writerConfig 'UPDATE appdb.orders SET amount = 2.00 WHERE id = 99'
    Check 'E3 DML connection can UPDATE' ($w2.exit -eq 0) ($w2.text -replace '\s+', ' ')

    $w3 = Invoke-Query $writerConfig 'DROP TABLE appdb.orders'
    Check 'E4 DML level still rejects DROP TABLE' ($w3.exit -ne 0) ($w3.text -replace '\s+', ' ')

    $w4 = Invoke-Query $writerConfig 'CREATE TABLE appdb.tmp_should_fail (id INT)'
    Check 'E5 DML level rejects CREATE TABLE' ($w4.exit -ne 0) ($w4.text -replace '\s+', ' ')

    $w5 = Invoke-Query $writerConfig 'DELETE FROM appdb.orders WHERE id = 99'
    Check 'E6 DML connection can DELETE' ($w5.exit -eq 0) ($w5.text -replace '\s+', ' ')

    $verify = Invoke-Query $writerConfig 'SELECT COUNT(*) AS c FROM appdb.orders'
    $count = Get-FirstCell $verify 'c'
    Check 'E7 write verified by read-back (back to 3 rows)' ("$count" -eq '3') ($verify.text -replace '\s+', ' ')

    $w6 = Invoke-Query $writerConfig "GRANT ALL ON *.* TO 'dbo_reader'@'%'"
    Check 'E8 GRANT rejected' ($w6.exit -ne 0) ($w6.text -replace '\s+', ' ')

    $w7 = Invoke-Query $writerConfig 'SELECT @@version_comment'
    Check 'E9 session variable query still rejected for DML' ($w7.exit -ne 0) ($w7.text -replace '\s+', ' ')

    # ---------- Phase F: management UI on the real database ----------
    $env:DB_OPERATOR_HOME = $privateDir
    $env:DB_OPERATOR_UI_PORT = "$uiPort"
    $env:DB_OPERATOR_ENDPOINT = $endpoint
    $uiLog = Join-Path $work 'ui.log'
    $uiErr = Join-Path $work 'ui.err.log'
    $script:uiProcess = Start-Process -FilePath (Get-Command node).Source `
        -ArgumentList @("`"$uiServer`"") -WorkingDirectory $Package `
        -WindowStyle Hidden -RedirectStandardOutput $uiLog -RedirectStandardError $uiErr -PassThru
    $uiReady = $false
    $session = $null
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 250
        try { $session = Invoke-RestMethod "http://127.0.0.1:$uiPort/api/session" -TimeoutSec 1; $uiReady = $true; break } catch { }
        if ($script:uiProcess.HasExited) { break }
    }
    Check 'F1 management UI serves a session' ($uiReady -and $session.service -eq 'db-operator' -and $session.platform -eq 'windows' -and -not $session.requires_agent_user) (Get-Content -LiteralPath $uiErr -Raw -ErrorAction SilentlyContinue)

    $headers = @{ 'x-db-operator-ui-token' = $session.token; 'content-type' = 'application/json' }
    function Ui([string]$Path, [string]$Method = 'GET', $Body = $null) {
        $params = @{ Uri = "http://127.0.0.1:$uiPort$Path"; Method = $Method; Headers = $headers; TimeoutSec = 60 }
        if ($null -ne $Body) { $params.Body = ($Body | ConvertTo-Json -Depth 8 -Compress) }
        try { return [pscustomobject]@{ status = 200; data = (Invoke-RestMethod @params) } }
        catch {
            $resp = $_.Exception.Response
            $code = if ($resp) { [int]$resp.StatusCode } else { 0 }
            $payload = ''
            if ($resp) { $reader = [IO.StreamReader]::new($resp.GetResponseStream()); $payload = $reader.ReadToEnd() }
            return [pscustomobject]@{ status = $code; data = ($payload | ConvertFrom-Json) }
        }
    }

    $noTokenStatus = 0
    try { Invoke-RestMethod "http://127.0.0.1:$uiPort/api/connections" -TimeoutSec 5 | Out-Null; $noTokenStatus = 200 }
    catch { $noTokenStatus = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 } }
    Check 'F2 list without token rejected' ($noTokenStatus -eq 403) ("status=$noTokenStatus")
    $uiList = Ui '/api/connections'
    Check 'F3 UI lists the two registered connections' ($uiList.status -eq 200 -and $uiList.data.connections.Count -eq 2)

    $uiProfile = @{
        engine = 'mysql'; host = '127.0.0.1'; port = $MysqlPort; database = 'appdb'; username = 'dbo_reader'
        ssl_mode = 'disable'; environment = 'ui'; tags = @('e2e'); write_level = 'none'
        connect_timeout = 5; statement_timeout = 15; max_rows = 50
    }
    $uiClient = Join-Path $uiDir 'client.json'
    $uiCreate = Ui '/api/connections' 'POST' @{ name = 'shop-ui'; profile = $uiProfile; password = 'reader-secret-1'; test = $true; issue = $false }
    Check 'F4 UI register with real connection test' ($uiCreate.status -eq 200 -and $uiCreate.data.saved -and $uiCreate.data.tested) ($uiCreate.data | ConvertTo-Json -Compress)

    $uiShow = Ui '/api/connections/shop-ui'
    Check 'F5 UI detail hides the password' ($uiShow.status -eq 200 -and -not (($uiShow.data | ConvertTo-Json -Depth 6) -match 'reader-secret-1') -and $uiShow.data.profile.database -eq 'appdb')

    $uiUpdatedProfile = @{}
    foreach ($key in $uiProfile.Keys) { $uiUpdatedProfile[$key] = $uiProfile[$key] }
    $uiUpdatedProfile.max_rows = 25
    $uiUpdatedProfile.environment = 'ui-updated'
    $uiUpdate = Ui '/api/connections/shop-ui' 'PUT' @{ profile = $uiUpdatedProfile; password = ''; test = $true }
    $uiShow2 = Ui '/api/connections/shop-ui'
    Check 'F6 UI edit keeps the stored password and applies changes' ($uiUpdate.status -eq 200 -and $uiShow2.data.profile.max_rows -eq 25 -and $uiShow2.data.profile.environment -eq 'ui-updated') (($uiShow2.data.profile | ConvertTo-Json -Compress))

    $uiBadPass = Ui '/api/connections' 'POST' @{ name = 'shop-ui-bad'; profile = $uiProfile; password = 'totally-wrong'; test = $true; issue = $false }
    Check 'F7 UI failed test refuses to save with a readable error' ($uiBadPass.status -eq 400 -and $uiBadPass.data.error -match 'Access denied' -and $uiBadPass.data.error -notmatch 'dbo_reader' -and $uiBadPass.data.error -notmatch '\w<redacted>\w') ($uiBadPass.data.error)

    $uiOffProfile = @{}
    foreach ($key in $uiProfile.Keys) { $uiOffProfile[$key] = $uiProfile[$key] }
    $uiOffProfile.port = 33399
    $uiUnreachable = Ui '/api/connections' 'POST' @{ name = 'shop-ui-off'; profile = $uiOffProfile; password = 'reader-secret-1'; test = $true; issue = $false }
    Check 'F8 UI unreachable host error stays readable' ($uiUnreachable.status -eq 400 -and $uiUnreachable.data.error -match 'timed out|timeout|refused') ($uiUnreachable.data.error)

    $uiList2 = Ui '/api/connections'
    Check 'F9 failed registrations are not stored' ($uiList2.data.connections.Count -eq 3) (($uiList2.data.connections | ForEach-Object { $_.name }) -join ',')

    # auto-issue on create is skipped while another connection holds the slot
    $uiAuto = Ui '/api/connections' 'POST' @{ name = 'shop-ui-2'; profile = $uiProfile; password = 'reader-secret-1'; test = $false; issue = $true }
    Check 'F10 auto-issue reports the single-slot conflict instead of stealing it' ($uiAuto.status -eq 200 -and $uiAuto.data.issue_skipped -eq 'capability-exists' -and $uiAuto.data.existing_connection) ($uiAuto.data | ConvertTo-Json -Compress)

    $uiIssue = Ui '/api/capability/issue' 'POST' @{ connection = 'shop-ui'; endpoint = $endpoint; output = $uiClient; restart = $false }
    Check 'F11a UI issuance mirrors the registered write level' ($uiIssue.status -eq 200 -and $uiIssue.data.write_level -eq 'none') ("write_level=" + $uiIssue.data.write_level)
    Check 'F11 UI issues authorization (no restart requested)' ($uiIssue.status -eq 200 -and $uiIssue.data.issued -and $uiIssue.data.service_restarted -eq $false -and (Test-Path -LiteralPath $uiClient)) ($uiIssue.data | ConvertTo-Json -Compress)

    $uiCap = Ui '/api/capability'
    Check 'F12 UI reports the active authorization and defaults' ($uiCap.status -eq 200 -and $uiCap.data.active -and $uiCap.data.connection -eq 'shop-ui' -and $uiCap.data.defaults.endpoint -like '*pipe*') (($uiCap.data | ConvertTo-Json -Compress))

    Stop-Daemon
    if (-not (Start-Daemon $uiClient)) { throw 'daemon did not become ready with the UI-issued capability' }
    $uiQuery = Invoke-Query $uiClient 'SELECT COUNT(*) AS c FROM appdb.orders'
    $uiCount = Get-FirstCell $uiQuery 'c'
    Check 'F13 UI-issued capability queries the real database' ($uiQuery.exit -eq 0 -and "$uiCount" -eq '3') ($uiQuery.text -replace '\s+', ' ')

    # A DML connection must receive a capability that actually carries dml; issuing at
    # the default level would silently keep it read-only.
    $writerUiClient = Join-Path $agentDir 'writer-ui.json'
    $uiWriterIssue = Ui '/api/capability/issue' 'POST' @{ connection = 'shop-write'; endpoint = $endpoint; output = $writerUiClient; restart = $false }
    Check 'F15 UI issuance mirrors the DML write level' ($uiWriterIssue.status -eq 200 -and $uiWriterIssue.data.write_level -eq 'dml') ("write_level=" + $uiWriterIssue.data.write_level)
    Stop-Daemon
    if (-not (Start-Daemon $writerUiClient)) { throw 'daemon did not become ready with the UI-issued writer capability' }
    $writerClient = $writerUiClient
    $ins = Invoke-Query $writerClient "INSERT INTO appdb.orders VALUES (99, 'mallory', 1.00)"
    $del = Invoke-Query $writerClient 'DELETE FROM appdb.orders WHERE id = 99'
    $drop = Invoke-Query $writerClient 'DROP TABLE appdb.orders'
    Check 'F16 DML capability writes through the UI-issued client' ($ins.exit -eq 0 -and $del.exit -eq 0 -and $drop.exit -ne 0) (($ins.text + ' | ' + $del.text + ' | ' + $drop.text) -replace '\s+', ' ')
    $afterWriteResult = Invoke-Query $writerClient 'SELECT COUNT(*) AS c FROM appdb.orders'
    $afterWrite = Get-FirstCell $afterWriteResult 'c'
    Check 'F17 writes verified by read-back' ("$afterWrite" -eq '3') ("rows=$afterWrite")

    $uiRefresh = Ui '/api/schema-refresh' 'POST' @{}
    Check 'F14 UI refreshes the schema cache' ($uiRefresh.status -eq 200) ($uiRefresh.data | ConvertTo-Json -Compress)

    # ---------- Phase G: delete cascades ----------
    $pending = Ui '/api/connections/shop-ui/delete-confirmation' 'POST' @{}
    $wrongName = Ui '/api/connections/shop-ui' 'DELETE' @{ confirmation = $pending.data.confirmation; confirmed_name = 'someone-else' }
    Check 'G1 delete with wrong confirmation name rejected' ($wrongName.status -eq 409)
    $deleted = Ui '/api/connections/shop-ui' 'DELETE' @{ confirmation = $pending.data.confirmation; confirmed_name = 'shop-ui' }
    Check 'G2 confirmed delete removes the connection' ($deleted.status -eq 200 -and $deleted.data.removed) ($deleted.data | ConvertTo-Json -Compress)
    # The active capability belongs to shop-write at this point, so deleting shop-ui must
    # leave that authorization (and its client config) alone.
    $capAfterUiDelete = Get-Content -LiteralPath (Join-Path $privateDir 'capability.json') -Raw | ConvertFrom-Json
    Check 'G3 deleting a connection keeps another connection authorization' ($deleted.data.capability_revoked -eq $false -and $capAfterUiDelete.connection -eq 'shop-write' -and (Test-Path -LiteralPath $writerUiClient)) ("capability=" + $capAfterUiDelete.connection)

    $uiList3 = Ui '/api/connections'
    $remaining = (($uiList3.data.connections | ForEach-Object { $_.name }) | Sort-Object) -join ','
    Check 'G4 other connections untouched' ($uiList3.data.connections.Count -eq 3 -and $remaining -eq 'shop-read,shop-ui-2,shop-write') ($remaining)

    # Deleting the connection that owns the capability must revoke it and remove client.json.
    $writerPending = Ui '/api/connections/shop-write/delete-confirmation' 'POST' @{}
    $writerDeleted = Ui '/api/connections/shop-write' 'DELETE' @{ confirmation = $writerPending.data.confirmation; confirmed_name = 'shop-write' }
    Check 'G5 deleting the authorizing connection revokes the capability' ($writerDeleted.status -eq 200 -and $writerDeleted.data.capability_revoked -eq $true -and -not (Test-Path -LiteralPath (Join-Path $privateDir 'capability.json'))) ($writerDeleted.data | ConvertTo-Json -Compress)

    $ghost = Invoke-DbOperator $writerUiClient 'status'
    Check 'G6 revoked client stops working' ($ghost.exit -ne 0) ($ghost.text -replace '\s+', ' ')

    $uiList4 = Ui '/api/connections'
    $afterDelete = (($uiList4.data.connections | ForEach-Object { $_.name }) | Sort-Object) -join ','
    Check 'G7 registry keeps the read-only connections' ($uiList4.data.connections.Count -eq 2 -and $afterDelete -eq 'shop-read,shop-ui-2') ($afterDelete)

    $dupCreate = Ui '/api/connections' 'POST' @{ name = 'shop-read'; profile = $uiProfile; password = 'reader-secret-1'; test = $false }
    Check 'G8 duplicate registration rejected with a readable error' ($dupCreate.status -eq 400 -and $dupCreate.data.error -match 'already exists') ($dupCreate.data.error)

    # ---------- Phase H: daemon down ----------
    Stop-Daemon
    $down = Invoke-DbOperator $clientConfig 'status'
    Check 'H1 client reports unavailability when the daemon stops' ($down.exit -ne 0) ($down.text -replace '\s+', ' ')
}
finally {
    Stop-Daemon
    if ($null -ne $script:uiProcess -and -not $script:uiProcess.HasExited) {
        Stop-Process -Id $script:uiProcess.Id -Force -ErrorAction SilentlyContinue
    }
    $env:DB_OPERATOR_HOME = $oldHome
    $env:DB_OPERATOR_CLIENT_CONFIG = $oldClient
    $env:DB_OPERATOR_UI_PORT = $null
    $env:DB_OPERATOR_ENDPOINT = $null
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
}

$failed = @($script:results | Where-Object { -not $_.ok })
Write-Output ''
Write-Output ("FLOW SUMMARY: {0}/{1} passed" -f ($script:results.Count - $failed.Count), $script:results.Count)
if ($failed.Count -gt 0) {
    Write-Output 'FAILED:'
    $failed | ForEach-Object { Write-Output ("  - {0}  {1}" -f $_.name, $_.detail) }
    exit 1
}
exit 0
