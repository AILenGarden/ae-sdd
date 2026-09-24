$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'open-registry.ps1') -NoBrowser
exit $LASTEXITCODE
