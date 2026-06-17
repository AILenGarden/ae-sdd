# install.ps1 — ae-sdd SKILL 安装脚本（Windows PowerShell）
#
# 支持两种模式：
#   远程模式（irm ... | iex）：自动下载 zip 后安装
#   本地模式（.\scripts\install.ps1）：需在仓库根目录执行
#
# 安装目标：$env:USERPROFILE\.claude\skills\ae-sdd
#
# 用法：
#   irm https://raw.githubusercontent.com/AILenGarden/ae-sdd/main/scripts/install.ps1 | iex
#   .\scripts\install.ps1

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$RepoUrl     = "https://github.com/AILenGarden/ae-sdd"
$ArchiveUrl  = "https://github.com/AILenGarden/ae-sdd/archive/refs/heads/main.zip"
$SkillName   = "ae-sdd"
$Dst         = Join-Path $env:USERPROFILE ".claude\skills\$SkillName"

# ─── 颜色输出 ────────────────────────────────────────────────────────────────
function Write-Info    { param($Msg) Write-Host "[ae-sdd] $Msg" -ForegroundColor Cyan }
function Write-Warn    { param($Msg) Write-Host "[ae-sdd] ⚠  $Msg" -ForegroundColor Yellow }
function Write-Err     { param($Msg) Write-Host "[ae-sdd] x  $Msg" -ForegroundColor Red }
function Write-Success { param($Msg) Write-Host "[ae-sdd] OK $Msg" -ForegroundColor Green }

# ─── 检测运行模式 ─────────────────────────────────────────────────────────────
function Detect-Mode {
  $LocalPlugin = Join-Path (Get-Location) "plugins\ae-sdd\SKILL.md"
  if (Test-Path $LocalPlugin) {
    $script:Src = (Get-Location).Path
    Write-Info "检测到本地仓库模式，使用 $script:Src"
    $script:TmpDir = $null
  } else {
    # 远程模式：下载 zip 解压
    $timestamp = Get-Date -Format "yyyyMMddHHmmss"
    $script:TmpDir = Join-Path $env:TEMP "ae-sdd-install-$timestamp"
    New-Item -ItemType Directory -Path $script:TmpDir -Force | Out-Null

    $ZipPath = Join-Path $script:TmpDir "ae-sdd.zip"
    Write-Info "远程模式：正在下载仓库..."

    try {
      Invoke-WebRequest -Uri $ArchiveUrl -OutFile $ZipPath -UseBasicParsing
    } catch {
      Write-Err "下载失败：$_"
      Write-Err "请检查网络，或手动 clone 仓库后执行 .\scripts\install.ps1"
      exit 1
    }

    Write-Info "解压中..."
    Expand-Archive -Path $ZipPath -DestinationPath $script:TmpDir -Force
    # GitHub zip 解压后子目录形如 ae-sdd-main
    $Extracted = Get-ChildItem -Path $script:TmpDir -Directory | Where-Object { $_.Name -like "ae-sdd-*" } | Select-Object -First 1
    if (-not $Extracted) {
      Write-Err "解压目录结构异常，未找到 ae-sdd-* 子目录"
      exit 1
    }
    $script:Src = $Extracted.FullName
    Write-Info "下载并解压完成"
  }
}

# ─── 备份旧版本 ───────────────────────────────────────────────────────────────
function Backup-Existing {
  if (Test-Path $Dst) {
    $timestamp = Get-Date -Format "yyyyMMddHHmmss"
    $Bak = "${Dst}.bak.$timestamp"
    Write-Warn "检测到已有安装版本，备份到："
    Write-Warn "  $Bak"
    Rename-Item -Path $Dst -NewName $Bak
  }
}

# ─── 复制文件 ─────────────────────────────────────────────────────────────────
function Install-Files {
  $PluginSrc = Join-Path $script:Src "plugins\ae-sdd"
  if (-not (Test-Path $PluginSrc)) {
    Write-Err "未找到 $PluginSrc，仓库结构异常"
    Cleanup
    exit 1
  }
  New-Item -ItemType Directory -Path $Dst -Force | Out-Null
  Copy-Item -Path "$PluginSrc\*" -Destination $Dst -Recurse -Force
  Write-Info "文件已复制到 $Dst"
}

# ─── 验证安装 ─────────────────────────────────────────────────────────────────
function Verify-Install {
  $SkillMd = Join-Path $Dst "SKILL.md"
  if (-not (Test-Path $SkillMd)) {
    Write-Err "安装验证失败：$SkillMd 不存在"
    Cleanup
    exit 1
  }
}

# ─── 清理临时目录 ─────────────────────────────────────────────────────────────
function Cleanup {
  if ($script:TmpDir -and (Test-Path $script:TmpDir)) {
    Remove-Item -Recurse -Force $script:TmpDir
  }
}

# ─── 打印使用提示 ─────────────────────────────────────────────────────────────
function Print-Usage {
  Write-Host ""
  Write-Success "ae-sdd SKILL 安装成功！"
  Write-Host ""
  Write-Host "  安装路径：$Dst"
  Write-Host ""
  Write-Host "  在 Claude Code 中使用："
  Write-Host "    输入  /ae-sdd  启动自动化工程助手"
  Write-Host ""
  Write-Host "  更多信息：$RepoUrl"
  Write-Host ""
}

# ─── 主流程 ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Info "开始安装 ae-sdd SKILL..."
Write-Host ""

$script:Src    = $null
$script:TmpDir = $null

Detect-Mode
Backup-Existing
Install-Files
Verify-Install
Cleanup
Print-Usage
