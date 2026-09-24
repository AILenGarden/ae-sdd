param(
    [switch]$BuildFromSource
)

$ErrorActionPreference = 'Stop'
$skillDir = Split-Path -Parent $PSScriptRoot
$binDir = Join-Path $skillDir 'bin'
$destination = Join-Path $binDir 'db-operator.exe'
$architecture = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
$prebuilt = Join-Path $binDir "windows-$architecture\db-operator.exe"

New-Item -ItemType Directory -Force -Path $binDir | Out-Null

if ((-not $BuildFromSource) -and (Test-Path -LiteralPath $prebuilt)) {
    Copy-Item -LiteralPath $prebuilt -Destination $destination -Force
} elseif (-not (Test-Path -LiteralPath $destination) -or $BuildFromSource) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        throw "No prebuilt Agent client exists for windows-$architecture and Cargo is not installed."
    }
    & $cargo.Source build --release --no-default-features --features client --bin db-operator --manifest-path (Join-Path $skillDir 'native\Cargo.toml')
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo client build failed with exit code $LASTEXITCODE."
    }
    Copy-Item -LiteralPath (Join-Path $skillDir 'native\target\release\db-operator.exe') -Destination $destination -Force
}

& $destination --version
Write-Output "Installed query-only db-operator client: $destination"
Write-Output 'Client installation only: the daemon service and client.json are not configured by this script.'
Write-Output 'The full Windows x64 release includes service components. An administrator should follow references/connection-schema.md and run the bundled scripts/install-service.ps1.'
Write-Output 'If service files are absent, obtain or fully extract the matching full release; see the setup reference for required paths.'
