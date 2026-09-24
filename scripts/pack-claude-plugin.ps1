#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Pack ae-sdd source into Claude Code plugin format.

.DESCRIPTION
    Converts ae-sdd (Codex format) to ae-sdd-claude (Claude Code format).
    - Copies SKILL.md files with path adaptations
    - Adds version to frontmatter
    - Copies reference documents
    - Generates manifest.json
    - Creates directory structure

.PARAMETER SourceRoot
    Source directory (default: ../ae-sdd relative to script)

.PARAMETER TargetRoot
    Target directory (default: ../ae-sdd-claude)

.PARAMETER Version
    Plugin version (default: read from source plugin.json or 0.1.0)

.PARAMETER Clean
    Clean target directory before packing

.EXAMPLE
    .\pack-claude-plugin.ps1
    Pack with default settings

.EXAMPLE
    .\pack-claude-plugin.ps1 -Clean -Version "0.2.0"
    Clean rebuild with specific version
#>

[CmdletBinding()]
param(
    [string]$SourceRoot,
    [string]$TargetRoot,
    [string]$Version,
    [string]$Python = 'python',
    [switch]$Clean
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Resolve paths
$ScriptDir = Split-Path -Parent $PSCommandPath
$DefaultSourceRoot = Join-Path $ScriptDir '..' | Resolve-Path
$DefaultTargetRoot = Join-Path $DefaultSourceRoot '../ae-sdd-claude'

$SourceRoot = if ($SourceRoot) { $SourceRoot } else { $DefaultSourceRoot }
$TargetRoot = if ($TargetRoot) { $TargetRoot } else { $DefaultTargetRoot }
$SourceRoot = [IO.Path]::GetFullPath([string]$SourceRoot)
$TargetRoot = [IO.Path]::GetFullPath([string]$TargetRoot)

Write-Host "[ae-sdd Claude Code Plugin Packer]" -ForegroundColor Cyan
Write-Host "   Source: $SourceRoot" -ForegroundColor Gray
Write-Host "   Target: $TargetRoot" -ForegroundColor Gray

# Detect version
if (-not $Version) {
    $SourcePluginJson = Join-Path $SourceRoot '.codex-plugin/plugin.json'
    if (Test-Path $SourcePluginJson) {
        $PluginData = Get-Content $SourcePluginJson | ConvertFrom-Json
        $Version = $PluginData.version
        Write-Host "   Version: $Version (from source)" -ForegroundColor Gray
    } else {
        $Version = '0.1.0'
        Write-Host "   Version: $Version (default)" -ForegroundColor Yellow
    }
}

# Clean if requested
if ($Clean -and (Test-Path $TargetRoot)) {
    Write-Host "[Cleaning] target directory..." -ForegroundColor Yellow
    Remove-Item $TargetRoot -Recurse -Force
}

# Create directory structure
Write-Host "[Step 1/7] Creating directory structure..." -ForegroundColor Green
$Directories = @(
    $TargetRoot,
    (Join-Path $TargetRoot 'references'),
    (Join-Path $TargetRoot 'scripts')
)
foreach ($Dir in $Directories) {
    if (-not (Test-Path $Dir)) {
        New-Item -ItemType Directory -Path $Dir -Force | Out-Null
    }
}

# Helper: Add version to frontmatter
function Add-VersionToFrontmatter {
    param([string]$Content, [string]$Version)

    if ($Content -match '^---\r?\n(.*?)\r?\n---\r?\n(.*)$') {
        $Frontmatter = $matches[1]
        $Body = $matches[2]

        # Check if version already exists
        if ($Frontmatter -match 'version:') {
            $Frontmatter = $Frontmatter -replace 'version:.*', "version: $Version"
        } else {
            $Frontmatter += "`nversion: $Version"
        }

        return "---`n$Frontmatter`n---`n$Body"
    }
    return $Content
}

# Helper: Adapt paths in content
function Adapt-Paths {
    param([string]$Content, [string]$Context)

    switch ($Context) {
        'main-skill' {
            $Content = $Content -replace '\.\./\.\./capability-catalog\.md', './references/capability-catalog.md'
            $Content = $Content -replace '\.\./\.\./references/install\.md', './references/install.md'
            $Content = $Content -replace '\.\./\.\./references/uninstall\.md', './references/uninstall.md'
        }
        'install-skill' {
            $Content = $Content -replace 'references/agents-guidance\.md', '../../references/agents-guidance.md'
        }
        'uninstall-skill' {
            $Content = $Content -replace 'references/agents-guidance\.md', '../../references/agents-guidance.md'
        }
    }

    return $Content
}

# Pack main SKILL.md
# Bundle private knowledge before publishing the public entry. Python must have
# the capability's PyYAML dependency; pass -Python to select an existing venv.
& $Python (Join-Path $SourceRoot 'scripts/bundle_knowledge.py') --output (Join-Path $TargetRoot 'capabilities/al-knowledge')
if ($LASTEXITCODE -ne 0) { throw 'Private knowledge bundle build failed; target preserved for inspection.' }

Write-Host "[Step 2/7] Packing main SKILL.md..." -ForegroundColor Green
$MainSkillSource = Join-Path $SourceRoot 'skills/ae-sdd/SKILL.md'
$MainSkillTarget = Join-Path $TargetRoot 'SKILL.md'
if (Test-Path $MainSkillSource) {
    $Content = Get-Content $MainSkillSource -Raw -Encoding UTF8
    $Content = Add-VersionToFrontmatter $Content $Version
    $Content = Adapt-Paths $Content 'main-skill'
    [System.IO.File]::WriteAllText($MainSkillTarget, $Content, [System.Text.Encoding]::UTF8)
    Write-Host "   [OK] SKILL.md" -ForegroundColor DarkGray
} else {
    Write-Warning "Main SKILL.md not found: $MainSkillSource"
}

# Installation and removal are copied below as reference documents.
Copy-Item -LiteralPath (Join-Path $SourceRoot 'scripts/manage_agents.py') -Destination (Join-Path $TargetRoot 'scripts/manage_agents.py')

# Copy references
Write-Host "[Step 5/7] Copying references..." -ForegroundColor Green
$RefDocs = @(
    'install.md',
    'uninstall.md',
    'agents-guidance.md',
    'installation-contract.md',
    'gui-management.md'
)
$RefSourceDir = Join-Path $SourceRoot 'references'
$RefTargetDir = Join-Path $TargetRoot 'references'
foreach ($Doc in $RefDocs) {
    $SourcePath = Join-Path $RefSourceDir $Doc
    $TargetPath = Join-Path $RefTargetDir $Doc
    if (Test-Path $SourcePath) {
        Copy-Item $SourcePath $TargetPath -Force
        Write-Host "   [OK] references/$Doc" -ForegroundColor DarkGray
    }
}

# Copy capability-catalog.md
$CapCatalogSource = Join-Path $SourceRoot 'capability-catalog.md'
$CapCatalogTarget = Join-Path $RefTargetDir 'capability-catalog.md'
if (Test-Path $CapCatalogSource) {
    Copy-Item $CapCatalogSource $CapCatalogTarget -Force
    Write-Host "   [OK] references/capability-catalog.md" -ForegroundColor DarkGray
}

# Generate README.md
Write-Host "[Step 6/9] Generating README.md..." -ForegroundColor Green
$ReadmeContent = @"
# ae-sdd - AL Engineering Capability Discovery & Routing

**Version**: $Version
**Type**: Claude Code Plugin Adapter
**Source**: ``al-skills/coding/al/ae-sdd/``

## 概述

``ae-sdd`` (AL Engineering Software Development Discovery) 是 AL 工程能力族的统一入口和路由层，负责根据任务类型自动选择和加载合适的工程能力。

它是一个轻量级的能力发现和路由组件，**不是**流程编排器、状态机或 daemon 服务。

## 核心功能

### 1. 能力路由
根据用户需求自动路由到五个核心工程能力之一：

| 能力 | 职责 | 典型场景 |
|------|------|----------|
| **al-ra** | 需求分析与澄清 | 需求模糊、边界不清、风险识别 |
| **al-spec** | 规格文档编写 | 编写 DR、Story、TestCase |
| **al-coding** | 代码实现与审查 | 编码、设计、Review、验证 |
| **al-knowledge** | ae-sdd 内置项目知识 | 随包提供，用户无需单独调用或安装 |
| **db-operator** | 数据库只读查询 | Schema 检查、数据提取、查询验证 |

### 2. 托管安装
- **ae-sdd 安装流程**: 统一安装和更新入口，支持托管式部署
- **ae-sdd 卸载流程**: 按安装记录卸载，保留用户修改和独立能力

### 3. 能力注册管理
- 通过 ``registry.yaml`` 统一管理能力注册表
- 支持独立安装的能力复用
- 提供 Web UI 进行能力配置和管理

## 快速开始

### 使用路由
直接描述你的任务，ae-sdd 会自动选择合适的能力：

``````
# 需求分析
"帮我分析一下用户登录的需求边界"
-> ae-sdd 自动路由到 al-ra

# 代码实现
"实现一个用户认证的中间件"
-> ae-sdd 自动路由到 al-coding

# 数据库查询
"查询 users 表的结构"
-> ae-sdd 自动路由到 db-operator
``````

### 安装插件
调用安装 Skill：
``````
ae-sdd 安装流程
``````

### 卸载插件
调用卸载 Skill：
``````
ae-sdd 卸载流程
``````

## 架构说明

### 插件版本说明
本目录 (``ae-sdd-claude/``) 是专门为 **Claude Code** 打包的插件适配版本：

- **源码目录**: ``al-skills/coding/al/ae-sdd/`` (Codex 格式)
- **Claude 适配**: ``al-skills/coding/al/ae-sdd-claude/`` (本目录)
- **部署目标**: ``al-skills/_agents/claude/ae-sdd/``

### 路由机制
1. **用户输入** -> ae-sdd 主入口
2. **任务分类** -> 读取 ``capability-catalog.md`` 匹配规则
3. **能力加载** -> 定位 ``registry.yaml`` 中的能力路径
4. **执行委托** -> 加载目标能力的 SKILL.md

### 与其他组件关系

``````
用户请求
    |
ae-sdd (路由层)
    |
    +-- al-ra (需求分析)
    +-- al-spec (规格编写)
    +-- al-coding (代码实现)
    +-- al-knowledge (知识管理)
    +-- db-operator (数据库查询)
``````

**关键特性**：
- 无中央状态机，各能力可独立调用
- 无强制流程，按需组合能力
- 无 daemon 服务，纯 Skill 加载机制

## 目录结构

``````
ae-sdd-claude/
+-- SKILL.md                      # 主入口（能力路由）
+-- README.md                     # 本文档
+-- manifest.json                 # 插件元数据
+-- references/                   # 参考文档（6个文件）
|   +-- agents-guidance.md
|   +-- capability-catalog.md
|   +-- installation-contract.md
|   +-- gui-management.md
|   +-- install.md        # ae-sdd 安装流程
|   +-- uninstall.md      # ae-sdd 卸载流程
+-- scripts/                     # 脚本说明（实际脚本在源码目录）
    +-- README.md
``````

## 能力边界

### ae-sdd 负责
- 根据任务类型选择合适的能力
- 读取注册表定位能力路径
- 托管式安装和卸载管理
- 能力清单和路由规则维护

### ae-sdd 不负责
- 流程编排和状态管理（由各能力自治）
- 中央事件总线或全局任务注册
- 代替各能力执行具体任务
- 在主入口复制知识模型细节（具体任务由随包的 al-knowledge 能力执行）

## 开发者指南

### 从源码同步
本插件是从源码目录适配而来，更新流程：

1. **源码更新后重新打包**：
   ``````powershell
   cd al-skills/coding/al/ae-sdd
   .\scripts\pack-claude-plugin.ps1
   ``````

2. **清理重建**：
   ``````powershell
   .\scripts\pack-claude-plugin.ps1 -Clean
   ``````

3. **指定版本**：
   ``````powershell
   .\scripts\pack-claude-plugin.ps1 -Version "0.2.0"
   ``````

### 打包器功能
``pack-claude-plugin.ps1`` 会自动：
- 复制并适配 SKILL.md 文件
- 添加版本号到 frontmatter
- 复制 references 文档
- 生成 manifest.json
- 生成本 README.md
- 验证输出完整性

## 依赖说明

### 运行时依赖
- **项目知识**: 内置 ``capabilities/al-knowledge/CAPABILITY.md`` 及完整资源，知识路径无需外部注册表或独立知识插件
- **其他工程能力**: 经 registry.yaml 解析 al-ra、al-spec、al-coding、db-operator，保留现有安装方式

### 可选依赖
- **Node.js**: 运行注册服务器（registry-server.mjs）
- **PowerShell**: 运行安装脚本（install-runtime.ps1）
- **Python + PyYAML**: 运行 al-coding 注册器

## 常见问题

### Q: ae-sdd 和五个能力是什么关系？
A: 用户统一使用 ae-sdd，由它按任务选择执行能力。知识能力 al-knowledge 已随包内置，其余能力按注册路径加载。

### Q: 必须按顺序调用能力吗？
A: 不必须。用户只需使用 ae-sdd，Agent 根据任务选择能力组合，不强制固定顺序。

### Q: ae-sdd-claude 和 ae-sdd 有什么区别？
A:
- ``ae-sdd``: Codex 插件格式，源码目录
- ``ae-sdd-claude``: Claude Code 插件适配，专门为 Claude Code 打包

### Q: 如何更新到最新版本？
A: 调用 ``ae-sdd 安装流程``，它会检测已安装版本并更新托管文件。

### Q: 卸载会删除我的项目知识库吗？
A: 不会。ae-sdd 托管的内置知识工具随插件移除，项目 .al-knowledge 数据及已有独立安装仍保留。

## 技术支持

- **源码仓库**: ``al-skills/coding/al/ae-sdd/``
- **问题反馈**: 在项目仓库提交 Issue
- **开发者**: AILenGarden

## 版本历史

- **$Version**: 从源码自动生成
  - 使用 pack-claude-plugin.ps1 打包
  - 基于 ae-sdd 源码创建 Claude 插件适配
  - 支持能力路由、托管安装、卸载管理

## 许可证

MIT License

---

**Generated**: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
**Packager**: pack-claude-plugin.ps1
"@
$ReadmeTarget = Join-Path $TargetRoot 'README.md'
[System.IO.File]::WriteAllText($ReadmeTarget, $ReadmeContent, [System.Text.Encoding]::UTF8)
Write-Host "   [OK] README.md" -ForegroundColor DarkGray

# Generate scripts/README.md
Write-Host "[Step 7/9] Generating scripts/README.md..." -ForegroundColor Green
$ScriptsReadme = @"
# 脚本说明

``ae-sdd-claude`` 是纯插件适配版本，不包含可执行脚本的副本。

完整的可执行脚本和 Web UI 位于源码目录：``al-skills/coding/al/ae-sdd/``

## 脚本位置

| 脚本 | 位置 | 用途 |
|------|------|------|
| **registry-server.mjs** | ``../ae-sdd/scripts/registry-server.mjs`` | 注册管理服务器（Node.js） |
| **install-runtime.ps1** | ``../ae-sdd/scripts/install-runtime.ps1`` | db-operator 运行时安装（PowerShell） |
| **open-registry.ps1** | ``../ae-sdd/scripts/open-registry.ps1`` | 打开注册管理器（PowerShell） |
| **pack-claude-plugin.ps1** | ``../ae-sdd/scripts/pack-claude-plugin.ps1`` | 打包 Claude Code 插件 |

## Web UI

| 文件 | 位置 | 用途 |
|------|------|------|
| **index.html** | ``../ae-sdd/ui/index.html`` | 能力管理 Web 界面 |

## 打包说明

本目录是通过 ``pack-claude-plugin.ps1`` 从源码自动生成的。

### 重新打包

```````powershell
cd al-skills/coding/al/ae-sdd
.\scripts\pack-claude-plugin.ps1
```````

### 清理重建

```````powershell
.\scripts\pack-claude-plugin.ps1 -Clean
```````

### 指定版本

```````powershell
.\scripts\pack-claude-plugin.ps1 -Version "0.2.0"
```````

**生成时间**: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
**源码版本**: $Version
"@
$ScriptsReadmeTarget = Join-Path $TargetRoot 'scripts/README.md'
[System.IO.File]::WriteAllText($ScriptsReadmeTarget, $ScriptsReadme, [System.Text.Encoding]::UTF8)
Write-Host "   [OK] scripts/README.md" -ForegroundColor DarkGray

# Generate manifest.json
Write-Host "[Step 8/9] Generating manifest.json..." -ForegroundColor Green
$Manifest = @{
    id = 'ae-sdd-claude'
    name = 'ae-sdd'
    version = $Version
    description = 'AL engineering capability discovery, routing, and managed installation for Claude Code'
    author = @{ name = 'AILenGarden' }
    license = 'MIT'
    compiled = $false
    schema = 'claude-code-plugin/v1'
    entry = 'SKILL.md'
    source_path = '../ae-sdd/skills/ae-sdd/SKILL.md'
    package_type = 'claude-adapter'
    keywords = @('agent', 'capability', 'skill-registry', 'engineering', 'routing')
    skills = @(
        @{ name = 'ae-sdd'; entry = 'SKILL.md'; description = 'Top-level capability discovery and routing' }
    )
    capabilities = @('al-ra', 'al-spec', 'al-coding', 'al-knowledge', 'db-operator')
    references = @{
        capability_catalog = 'references/capability-catalog.md'
        agents_guidance = 'references/agents-guidance.md'
        installation_contract = 'references/installation-contract.md'
        gui_management = 'references/gui-management.md'
    }
    source_repository = 'al-skills/coding/al/ae-sdd'
    target_agents = @('claude-code')
    deployment_path = 'al-skills/_agents/claude/ae-sdd'
    generated_at = (Get-Date -Format 'o')
}
$ManifestJson = $Manifest | ConvertTo-Json -Depth 10
$ManifestTarget = Join-Path $TargetRoot 'manifest.json'
[System.IO.File]::WriteAllText($ManifestTarget, $ManifestJson, [System.Text.Encoding]::UTF8)
Write-Host "   [OK] manifest.json" -ForegroundColor DarkGray

# Reject stale discoverable Skills instead of silently shipping obsolete entries.
$SkillEntries = @(Get-ChildItem -LiteralPath $TargetRoot -Filter SKILL.md -Recurse -File)
if ($SkillEntries.Count -ne 1 -or $SkillEntries[0].FullName -ne (Join-Path $TargetRoot 'SKILL.md')) {
    throw 'Output contains stale Skill entries. Select a fresh target or review it before a clean rebuild.'
}

# Verify output
Write-Host "`n[Step 9/9] Verification..." -ForegroundColor Green
$RequiredFiles = @(
    'SKILL.md',
    'capabilities/al-knowledge/CAPABILITY.md',
    'capabilities/al-knowledge/scripts/knowledge.py',
    'capabilities/al-knowledge/references/consumption-contract.md',
    'README.md',
    'manifest.json',
    'references/capability-catalog.md',
    'references/agents-guidance.md',
    'references/installation-contract.md',
    'references/gui-management.md',
    'references/install.md',
    'references/uninstall.md',
    'scripts/README.md'
    'scripts/manage_agents.py'
)

$MissingFiles = @()
foreach ($File in $RequiredFiles) {
    $FilePath = Join-Path $TargetRoot $File
    if (-not (Test-Path $FilePath)) {
        $MissingFiles += $File
    }
}

if ($MissingFiles.Count -eq 0) {
    Write-Host "   All required files present [OK]" -ForegroundColor Green

    # Stats
    $TotalFiles = (Get-ChildItem $TargetRoot -Recurse -File).Count
    $TotalSize = (Get-ChildItem $TargetRoot -Recurse -File | Measure-Object -Property Length -Sum).Sum
    $TotalSizeKB = [Math]::Round($TotalSize / 1KB, 1)

    Write-Host "`nPackage Stats:" -ForegroundColor Cyan
    Write-Host "   Files: $TotalFiles" -ForegroundColor Gray
    Write-Host "   Size: $TotalSizeKB KB" -ForegroundColor Gray
    Write-Host "   Version: $Version" -ForegroundColor Gray
    Write-Host "`nPacking completed successfully!" -ForegroundColor Green
    Write-Host "   Target: $TargetRoot" -ForegroundColor Gray
} else {
    Write-Warning "Missing files detected:"
    $MissingFiles | ForEach-Object { Write-Host "   [X] $_" -ForegroundColor Red }
    exit 1
}
