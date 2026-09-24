$ErrorActionPreference = 'Stop'
$packageRoot = Split-Path -Parent $PSScriptRoot
$server = Join-Path $PSScriptRoot 'registry-server.mjs'
$node = Get-Command node -ErrorAction SilentlyContinue
if (-not $node) { throw 'Node.js is required to run the ae-sdd registry UI.' }
Start-Process -FilePath $node.Source -ArgumentList $server -WorkingDirectory $packageRoot -WindowStyle Hidden | Out-Null
Start-Sleep -Milliseconds 300
Start-Process 'http://127.0.0.1:17843/'
