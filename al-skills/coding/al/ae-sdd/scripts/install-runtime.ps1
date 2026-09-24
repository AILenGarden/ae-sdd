param(
    [string]$Connection = '',
    [string]$AgentConfigPath = "$env:APPDATA\db-operator\client.json"
)

$ErrorActionPreference = 'Stop'
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$script = $MyInvocation.MyCommand.Path
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $args = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $script + '"'))
    if ($Connection) { $args += @('-Connection', $Connection) }
    $args += @('-AgentConfigPath', ('"' + $AgentConfigPath + '"'))
    Write-Output 'Requesting Windows administrator approval for db-operator runtime setup...'
    $child = Start-Process powershell.exe -Verb RunAs -WindowStyle Hidden -WorkingDirectory (Split-Path -Parent $script) -ArgumentList $args -Wait -PassThru
    if (-not $child) { throw 'Windows administrator approval was not started.' }
    if ($child.ExitCode -ne 0) { throw "Elevated runtime setup failed with exit code $($child.ExitCode)." }
    exit $child.ExitCode
}

$dbRoot = Join-Path (Split-Path -Parent $script) '..\skills\db-operator'
if (-not (Test-Path (Join-Path $dbRoot 'scripts\install-service.ps1'))) {
    $dbRoot = Join-Path (Split-Path -Parent $script) '..\..\db-operator'
}
$serviceScript = Join-Path $dbRoot 'scripts\install-service.ps1'
$admin = Join-Path $dbRoot 'bin\service\windows-x86_64\db-operator-admin.exe'
if (-not $Connection) {
    $env:DB_OPERATOR_HOME = 'C:\ProgramData\db-operator'
    $list = & $admin list --json | ConvertFrom-Json
    $Connection = @($list.connections | Where-Object default | Select-Object -First 1).name
}
if (-not $Connection) { throw 'No registered db-operator connection was found.' }
$account = "$env:USERDOMAIN\$env:USERNAME"
& $serviceScript -AgentAccount $account -Connection $Connection -AgentConfigPath $AgentConfigPath
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Output "ae-sdd runtime installed and verified for connection $Connection."
