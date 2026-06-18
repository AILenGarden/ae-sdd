# install.ps1 — ae-sdd SKILL 安装（薄壳，Windows PowerShell）
#
# 🆕 v3.0.1 跨平台化（2026-06-18）：
#   旧：此文件包含完整 install 逻辑（内部调 bash scripts/build-dist.sh — 仍依赖 bash）
#   新：薄壳，仅做"找 Python + exec install.py"
#
# 真正的实现见 scripts/install.py（跨平台，零外部依赖）。
#
# 用法：
#   irm https://raw.githubusercontent.com/AILenGarden/ae-sdd/main/scripts/install.ps1 | iex
#   .\scripts\install.ps1
#   .\scripts\install.ps1 -FromBuild
#   .\scripts\install.ps1 -Uninstall

[CmdletBinding()]
param(
  [switch]$FromBuild,
  [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

# 找 Python — Windows 上常见：python / py / python3
$PythonCmd = $null
$PythonArgs = @()
foreach ($candidate in @("python", "py", "python3")) {
  $cmd = Get-Command $candidate -ErrorAction SilentlyContinue
  if ($cmd) {
    if ($candidate -eq "py") {
      $PythonCmd = "py"
      $PythonArgs = @("-3")
    } else {
      $PythonCmd = $candidate
      $PythonArgs = @()
    }
    break
  }
}

if (-not $PythonCmd) {
  Write-Host "❌ 致命：未找到 python / py / python3，请先安装 Python 3.8+" -ForegroundColor Red
  Write-Host "   下载: https://www.python.org/downloads/" -ForegroundColor Yellow
  exit 1
}

# 定位 install.py
# 优先用 $PSScriptRoot（PowerShell 3.0+ 标准，自动解析脚本所在目录）
# 回退 MyInvocation（兼容老版本）
if ($PSScriptRoot) {
  $ScriptDir = $PSScriptRoot
} else {
  $ScriptPath = $MyInvocation.MyCommand.Path
  $ScriptDir = if ($ScriptPath) { Split-Path -Parent $ScriptPath } else { Join-Path (Get-Location) "scripts" }
}

$InstallPy = Join-Path $ScriptDir "install.py"
if (-not (Test-Path $InstallPy)) {
  Write-Host "❌ 致命：未找到 $InstallPy" -ForegroundColor Red
  exit 1
}

# 构造参数
$PyArgs = @()
if ($FromBuild)  { $PyArgs += "--from-build" }
if ($Uninstall)  { $PyArgs += "--uninstall" }

# exec install.py
& $PythonCmd @($PythonArgs + $InstallPy + $PyArgs)
exit $LASTEXITCODE
