param(
    [Parameter(Mandatory = $true)]
    [string]$AgentAccount,
    [Parameter(Mandatory = $true)]
    [string]$Connection,
    [Parameter(Mandatory = $true)]
    [string]$AgentConfigPath,
    [string]$InstallDir = "$env:ProgramFiles\DB Operator",
    [string]$DataDir = "$env:ProgramData\db-operator",
    [switch]$Register,
    [switch]$BuildFromSource
)

$ErrorActionPreference = 'Stop'

function Get-VirtualServiceSid([string]$Name) {
    try {
        return ([Security.Principal.NTAccount]"NT SERVICE\$Name").Translate([Security.Principal.SecurityIdentifier]).Value
    } catch {
        $bytes = [Text.Encoding]::Unicode.GetBytes($Name.ToUpperInvariant())
        $hash = [Security.Cryptography.SHA1]::Create().ComputeHash($bytes)
        $parts = 0..4 | ForEach-Object { [BitConverter]::ToUInt32($hash, $_ * 4) }
        return 'S-1-5-80-' + ($parts -join '-')
    }
}

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run install-service.ps1 from an elevated PowerShell session.'
}

$skillDir = Split-Path -Parent $PSScriptRoot
$architecture = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
$serviceBin = Join-Path $skillDir "bin\service\windows-$architecture"
$adminSource = Join-Path $serviceBin 'db-operator-admin.exe'
$daemonSource = Join-Path $serviceBin 'db-operator-daemon.exe'

if ($BuildFromSource -or -not (Test-Path $adminSource) -or -not (Test-Path $daemonSource)) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) { throw 'Service binaries are missing and Cargo is not installed.' }
    & $cargo.Source build --release --no-default-features --features admin,daemon --bin db-operator-admin --bin db-operator-daemon --manifest-path (Join-Path $skillDir 'native\Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw "Cargo service build failed with exit code $LASTEXITCODE." }
    $adminSource = Join-Path $skillDir 'native\target\release\db-operator-admin.exe'
    $daemonSource = Join-Path $skillDir 'native\target\release\db-operator-daemon.exe'
}

New-Item -ItemType Directory -Force -Path $InstallDir, $DataDir | Out-Null
$adminDestination = Join-Path $InstallDir 'db-operator-admin.exe'
$daemonDestination = Join-Path $InstallDir 'db-operator-daemon.exe'
$existingService = Get-Service -Name 'db-operator' -ErrorAction SilentlyContinue
if ($existingService) {
    if ($existingService.Status -ne 'Stopped') {
        Stop-Service -Name 'db-operator' -ErrorAction Stop
        $existingService.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(30))
    }
    $existingService.Dispose()
}
Copy-Item $adminSource $adminDestination -Force
Copy-Item $daemonSource $daemonDestination -Force

$serviceIdentity = 'NT SERVICE\db-operator'
$serviceRunAccount = 'NT AUTHORITY\LocalService'
$agentSid = ([Security.Principal.NTAccount]$AgentAccount).Translate([Security.Principal.SecurityIdentifier]).Value
$serviceSid = Get-VirtualServiceSid 'db-operator'
$endpoint = '\\.\pipe\db-operator-v2'
$pipeSddl = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;$serviceSid)(A;;GRGW;;;$agentSid)"
& sc.exe delete db-operator 2>$null | Out-Null
for ($attempt = 0; $attempt -lt 30; $attempt++) {
    $remaining = Get-Service -Name 'db-operator' -ErrorAction SilentlyContinue
    if (-not $remaining) { break }
    $remaining.Dispose()
    Start-Sleep -Milliseconds 200
}
if (Get-Service -Name 'db-operator' -ErrorAction SilentlyContinue) { throw 'Service deletion did not complete.' }
$binaryPath = "`"$daemonDestination`" --service --home `"$DataDir`" --endpoint `"$endpoint`" --pipe-sddl `"$pipeSddl`""
$service = New-Service -Name 'db-operator' -BinaryPathName $binaryPath -DisplayName 'DB Operator Security Daemon' -Description 'Credential-isolated, strictly read-only database query daemon.' -StartupType Automatic
if (-not $service) { throw 'Failed to create db-operator Windows Service.' }
$scConfigCommand = 'sc.exe config db-operator obj= "' + $serviceRunAccount + '"'
& cmd.exe /d /s /c $scConfigCommand | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Failed to create db-operator Windows Service: $LASTEXITCODE" }
& sc.exe sidtype db-operator unrestricted | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Failed to enable the db-operator service SID: $LASTEXITCODE" }

& icacls $InstallDir /inheritance:r /grant:r 'SYSTEM:(OI)(CI)F' 'Administrators:(OI)(CI)F' "$serviceIdentity`:(OI)(CI)RX" | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Failed to secure $InstallDir" }
& icacls $DataDir /inheritance:r /grant:r 'SYSTEM:(OI)(CI)F' 'Administrators:(OI)(CI)F' "$serviceIdentity`:(OI)(CI)F" | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Failed to secure $DataDir" }

$env:DB_OPERATOR_HOME = $DataDir
$env:DB_OPERATOR_PIPE_SDDL = $pipeSddl
if ($Register) {
    & $adminDestination register
    if ($LASTEXITCODE -ne 0) { throw "Database connection registration failed with exit code $LASTEXITCODE." }
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $AgentConfigPath) | Out-Null
& $adminDestination client issue --connection $Connection --endpoint $endpoint --output $AgentConfigPath
if ($LASTEXITCODE -ne 0) { throw "Agent client capability issuance failed with exit code $LASTEXITCODE." }
& icacls $AgentConfigPath /inheritance:r /grant:r 'SYSTEM:F' 'Administrators:F' "$AgentAccount`:R" | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Failed to secure $AgentConfigPath" }

$scFailureCommand = 'sc.exe failure db-operator reset= 86400 actions= restart/5000/restart/15000/none/0'
& cmd.exe /d /s /c $scFailureCommand | Out-Null
& sc.exe start db-operator | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Failed to start db-operator Windows Service: $LASTEXITCODE" }

Write-Output "Installed DB Operator Windows Service with Agent SID $agentSid."
Write-Output "Agent client configuration: $AgentConfigPath"
