param(
    [string]$SkillDir = (Split-Path -Parent $PSScriptRoot),
    [int]$Samples = 100,
    [string]$NativeBenchmarkPath
)

$ErrorActionPreference = 'Stop'
$admin = Join-Path $SkillDir 'bin\service\windows-x86_64\db-operator-admin.exe'
$daemon = Join-Path $SkillDir 'bin\service\windows-x86_64\db-operator-daemon.exe'
$client = Join-Path $SkillDir 'bin\windows-x86_64\db-operator.exe'
$leaf = 'db-operator-e2e-' + [guid]::NewGuid().ToString('N')
$testRoot = Join-Path ([IO.Path]::GetTempPath()) $leaf
$privateDir = Join-Path $testRoot 'private'
$agentDir = Join-Path $testRoot 'agent'
New-Item -ItemType Directory -Force -Path $privateDir, $agentDir | Out-Null

$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$currentSid = ([Security.Principal.NTAccount]$currentIdentity).Translate(
    [Security.Principal.SecurityIdentifier]
).Value
& icacls $privateDir /inheritance:r /grant:r 'SYSTEM:(OI)(CI)F' 'Administrators:(OI)(CI)F' "$currentIdentity`:(OI)(CI)F" | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Failed to secure the E2E private directory.' }

$oldHome = $env:DB_OPERATOR_HOME
$oldClientConfig = $env:DB_OPERATOR_CLIENT_CONFIG
$daemonProcess = $null
try {
    $env:DB_OPERATOR_HOME = $privateDir
    'temporary-e2e-password' | & $admin register `
        --name e2e-reporting `
        --engine postgresql `
        --host invalid.example `
        --port 5432 `
        --database reporting `
        --username read_only_user `
        --ssl-mode require `
        --password-stdin 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'E2E registration failed.' }

    $pipeName = 'db-operator-v2-e2e-' + [guid]::NewGuid().ToString('N')
    $endpoint = '\\.\pipe\' + $pipeName
    $clientConfig = Join-Path $agentDir 'client.json'
    & $admin client issue `
        --connection e2e-reporting `
        --endpoint $endpoint `
        --output $clientConfig | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'E2E capability issue failed.' }

    $pipeSddl = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;$currentSid)"
    $stderrPath = Join-Path $testRoot 'daemon.stderr.log'
    $daemonProcess = Start-Process `
        -FilePath $daemon `
        -ArgumentList @('--home', $privateDir, '--endpoint', $endpoint, '--pipe-sddl', $pipeSddl) `
        -WindowStyle Hidden `
        -RedirectStandardError $stderrPath `
        -PassThru

    $env:DB_OPERATOR_CLIENT_CONFIG = $clientConfig
    $status = $null
    for ($index = 0; $index -lt 40; $index++) {
        Start-Sleep -Milliseconds 100
        $statusText = & $client status 2>$null
        if ($LASTEXITCODE -eq 0) {
            $status = $statusText | ConvertFrom-Json
            break
        }
        if ($daemonProcess.HasExited) {
            throw ('Daemon exited: ' + (Get-Content -LiteralPath $stderrPath -Raw))
        }
    }
    if ($null -eq $status) { throw 'Agent status did not become ready.' }

    $config = Get-Content -LiteralPath $clientConfig -Raw | ConvertFrom-Json
    function Invoke-E2EPipe([string]$Capability, [string]$RequestId) {
        $pipe = [IO.Pipes.NamedPipeClientStream]::new(
            '.',
            $pipeName,
            [IO.Pipes.PipeDirection]::InOut,
            [IO.Pipes.PipeOptions]::Asynchronous
        )
        try {
            $pipe.Connect(2000)
            $encoding = [Text.UTF8Encoding]::new($false)
            $writer = [IO.StreamWriter]::new($pipe, $encoding, 4096, $true)
            $reader = [IO.StreamReader]::new($pipe, $encoding, $false, 4096, $true)
            try {
                $request = @{
                    version = 2
                    request_id = $RequestId
                    capability = $Capability
                    op = 'status'
                } | ConvertTo-Json -Compress
                $writer.WriteLine($request)
                $writer.Flush()
                return ($reader.ReadLine() | ConvertFrom-Json)
            }
            finally {
                $writer.Dispose()
                $reader.Dispose()
            }
        }
        finally {
            $pipe.Dispose()
        }
    }

    $invalid = Invoke-E2EPipe 'invalid-capability' 'invalid-1'
    if ($invalid.error.code -ne 'AUTHENTICATION_FAILED') {
        throw 'Invalid capability was not rejected.'
    }

    $latencies = [Collections.Generic.List[double]]::new()
    for ($index = 0; $index -lt $Samples; $index++) {
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $response = Invoke-E2EPipe $config.capability ('perf-' + $index)
        $watch.Stop()
        if (-not $response.ok) { throw 'Warm IPC request failed.' }
        $latencies.Add($watch.Elapsed.TotalMilliseconds)
    }
    $sorted = $latencies | Sort-Object
    $p50 = $sorted[[math]::Floor(($sorted.Count - 1) * 0.50)]
    $p95 = $sorted[[math]::Floor(($sorted.Count - 1) * 0.95)]
    $daemonProcess.Refresh()
    $result = [ordered]@{
        status_connection = $status.connection
        status_engine = $status.engine
        status_state = $status.state
        invalid_capability = $invalid.error.code
        warm_ipc_p50_ms = [math]::Round($p50, 3)
        warm_ipc_p95_ms = [math]::Round($p95, 3)
        warm_working_set_mib = [math]::Round($daemonProcess.WorkingSet64 / 1MB, 2)
    }
    if ($NativeBenchmarkPath) {
        $native = & $NativeBenchmarkPath 500 | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0) { throw 'Native IPC benchmark failed.' }
        $result.native_ipc_samples = $native.samples
        $result.native_ipc_p50_ms = $native.warm_ipc_p50_ms
        $result.native_ipc_p95_ms = $native.warm_ipc_p95_ms
    }
    [pscustomobject]$result | ConvertTo-Json -Compress
}
finally {
    if ($null -ne $daemonProcess -and -not $daemonProcess.HasExited) {
        Stop-Process -Id $daemonProcess.Id -Force -ErrorAction SilentlyContinue
        $daemonProcess.WaitForExit(5000) | Out-Null
    }
    $env:DB_OPERATOR_HOME = $oldHome
    $env:DB_OPERATOR_CLIENT_CONFIG = $oldClientConfig

    $resolved = [IO.Path]::GetFullPath($testRoot)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (
        $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and
        (Split-Path -Leaf $resolved).StartsWith('db-operator-e2e-')
    ) {
        Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction SilentlyContinue
    }
}
