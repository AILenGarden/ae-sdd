#!/usr/bin/env node
/**
 * migrate-docs.mjs — ae-sdd 存量文档迁移工具
 *
 * 把 design/、.ae-task/、.ae-plan/、.spec/iterations/ 下的 .md 迁到 ae-sdd-doc/
 *
 * 跨平台：Windows / macOS / Linux（Node.js 18+）
 *
 * ⚠️ 默认 DRY-RUN 模式，不会修改任何文件
 * 使用 --execute 才会真正执行迁移
 *
 * 用法：
 *   node scripts/migrate-docs.mjs --target /path/to/project --dry-run
 *   node scripts/migrate-docs.mjs --target /path/to/project --execute
 *   node scripts/migrate-docs.mjs --target /path/to/project --date 2026-06-17 --execute
 *
 * 选项：
 *   --target <path>    目标工程根路径（必填）
 *   --date <YYYY-MM-DD> 迭代日期（默认 = 当前日期）
 *   --dry-run          仅生成报告，不执行（默认）
 *   --execute          实际执行迁移（不删除旧文件，仅复制到新路径）
 *   --author <name>    ChangeLog 作者（默认 = Claude）
 *   --help             显示帮助
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ========== 旧路径 → 新路径映射表（🔴 与 document-storage-skill §8.1 保持同步） ==========

const MIGRATION_RULES = [
  // design/dr/ → ae-sdd-doc/iterations/{date}/DR/
  {
    pattern: /^design\/dr\/[^/]+\/(.+)\.md$/,
    docType: 'DR',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/DR/${m[1]}-v1.0.md`,
  },
  // design/story/be/task/{STORY-ID}/... → Task/（必须在 StorySupplement 之前匹配）
  {
    pattern: /^design\/story\/be\/task\/([^\/]+)\/(.+)\.md$/,
    docType: 'Task',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/Task/${m[1]}/${m[2]}-v1.0.md`,
  },
  // design/story/be/coding/{STORY-ID}/... → Coding/
  {
    pattern: /^design\/story\/be\/coding\/([^\/]+)\/(.+)\.md$/,
    docType: 'Coding',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/Coding/${m[1]}/${m[2]}-v${detectCodingVersion(m[2])}.md`,
  },
  // design/story/be/review/{STORY-ID}/... → CR/
  {
    pattern: /^design\/story\/be\/review\/([^\/]+)\/(.+)\.md$/,
    docType: 'CR',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/CR/${m[1]}/${m[2]}-v1.0.md`,
  },
  // design/testcase/be/{STORY-ID}/... → Test/
  {
    pattern: /^design\/testcase\/be\/([^\/]+)\/(.+)\.md$/,
    docType: 'Test',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/Test/${m[1]}/${m[2]}-v1.0.md`,
  },
  // design/story/be/{STORY-ID}.md → Story/（STORY-XXX 或 STORY-XXX-YY 格式）
  {
    pattern: /^design\/story\/be\/(STORY-[A-Z0-9]+(?:-[A-Z0-9]+)?)\.md$/,
    docType: 'Story',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/Story/${m[1]}-v1.0.md`,
  },
  // design/story/be/{STORY-ID}/...（Story 子目录文件，如 Supplement/WriterReport）
  {
    pattern: /^design\/story\/be\/(STORY-[A-Z0-9]+(?:-[A-Z0-9]+)?)\/(.+)\.md$/,
    docType: 'StorySupplement',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/Story/${m[1]}/${m[3]}.md`,
  },
  // .ae-task/Task-{事务简称}/... → Task/
  {
    pattern: /^\.ae-task\/Task-[^\/]+\/(.+)\.md$/,
    docType: 'Task',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/Task/${m[1]}-v1.0.md`,
  },
  // .ae-plan/Plan-{事务简称}/... → Coding/
  {
    pattern: /^\.ae-plan\/Plan-[^\/]+\/(.+)\.md$/,
    docType: 'Coding',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/Coding/${m[1]}-v1.0.md`,
  },
  // .spec/iterations/{iter}/{DocType}/... → {DocType}/
  {
    pattern: /^\.spec\/iterations\/[^\/]+\/([^\/]+)\/(.+)\.md$/,
    docType: 'PRD-or-Other',
    targetTemplate: (m, ctx) => `iterations/${ctx.date}/${m[1]}/${m[2]}-v1.0.md`,
  },
];

/**
 * 检测 Coding 报告的版本号（v{N}-r{M}）
 * 默认 v1.0，如果不是 v{N}-r{M} 模式则回退 v1.0
 */
function detectCodingVersion(fileName) {
  const match = fileName.match(/-v(\d+)(?:-r(\d+))?/);
  if (match) {
    return match[2] ? `${match[1]}.${match[2]}` : `${match[1]}.0`;
  }
  return '1.0';
}

// ========== CLI 参数解析 ==========

function parseArgs(argv) {
  const args = {
    target: null,
    date: new Date().toISOString().slice(0, 10), // 默认 = 当前日期 YYYY-MM-DD
    dryRun: true,  // 🔴 默认 DRY-RUN
    execute: false,
    author: 'Claude',
    help: false,
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case '--target':
        args.target = argv[++i];
        break;
      case '--date':
        args.date = argv[++i];
        break;
      case '--execute':
        args.dryRun = false;
        args.execute = true;
        break;
      case '--dry-run':
        args.dryRun = true;
        args.execute = false;
        break;
      case '--author':
        args.author = argv[++i];
        break;
      case '--help':
      case '-h':
        args.help = true;
        break;
      default:
        console.error(`⚠️  未知参数: ${arg}`);
    }
  }

  return args;
}

function showHelp() {
  console.log(`
migrate-docs.mjs — ae-sdd 存量文档迁移工具

用法：
  node scripts/migrate-docs.mjs --target <path> [--date YYYY-MM-DD] [--execute]

选项：
  --target <path>    目标工程根路径（必填）
  --date <YYYY-MM-DD> 迭代日期（默认 = 当前日期）
  --dry-run          仅生成报告，不执行（默认）
  --execute          实际执行迁移（不删除旧文件，仅复制到新路径）
  --author <name>    ChangeLog 作者（默认 = Claude）
  --help, -h         显示帮助

旧路径映射（与 document-storage-skill §8.1 保持同步）：
  design/dr/                       → ae-sdd-doc/iterations/{date}/DR/
  design/story/be/*.md             → ae-sdd-doc/iterations/{date}/Story/
  design/story/be/task/*/          → ae-sdd-doc/iterations/{date}/Task/{STORY-ID}/
  design/story/be/coding/*/        → ae-sdd-doc/iterations/{date}/Coding/{STORY-ID}/
  design/story/be/review/*/        → ae-sdd-doc/iterations/{date}/CR/{STORY-ID}/
  design/testcase/be/*/            → ae-sdd-doc/iterations/{date}/Test/{STORY-ID}/
  .ae-task/Task-*/*.md             → ae-sdd-doc/iterations/{date}/Task/
  .ae-plan/Plan-*/*.md             → ae-sdd-doc/iterations/{date}/Coding/
  .spec/iterations/{iter}/{type}/  → ae-sdd-doc/iterations/{date}/{type}/

⚠️  旧文件不会删除，仅复制到新路径。请人工确认后再手动清理。
`);
}

// ========== 核心：扫描 + 匹配 + 报告生成 ==========

function scanProject(rootPath) {
  const found = [];

  function walk(relPath) {
    const fullPath = path.join(rootPath, relPath);
    if (!fs.existsSync(fullPath)) return;

    const stat = fs.statSync(fullPath);
    if (stat.isFile()) {
      if (relPath.endsWith('.md') && !relPath.includes('node_modules')) {
        found.push(relPath.replace(/\\/g, '/'));
      }
      return;
    }

    if (stat.isDirectory()) {
      // 跳过 ae-sdd-doc/ 自身（新路径不扫描）
      if (relPath === 'ae-sdd-doc' || relPath.startsWith('ae-sdd-doc/')) return;
      // 跳过 .git
      if (relPath === '.git' || relPath.startsWith('.git/')) return;
      try {
        for (const entry of fs.readdirSync(fullPath)) {
          walk(path.join(relPath, entry).replace(/\\/g, '/'));
        }
      } catch (e) {
        // 权限问题等
      }
    }
  }

  walk('');
  return found;
}

function classifyFile(relPath) {
  for (const rule of MIGRATION_RULES) {
    const m = relPath.match(rule.pattern);
    if (m) {
      return { rule, match: m, docType: rule.docType };
    }
  }
  return null;
}

function buildMigrationPlan(rootPath, date, author) {
  const allFiles = scanProject(rootPath);
  const plan = [];
  const skipped = [];

  for (const relPath of allFiles) {
    const classified = classifyFile(relPath);
    if (!classified) {
      skipped.push(relPath);
      continue;
    }

    const { rule, match, docType } = classified;
    const targetRel = `ae-sdd-doc/${rule.targetTemplate(match, { date })}`;

    // 提取 docId（用于报告展示，路径中可见的稳定标识）
    let docId = match[1];
    if (docType === 'Task' && match[2]) docId = match[2];
    else if (docType === 'Coding' && match[2]) docId = match[2];
    else if (docType === 'CR' && match[2]) docId = match[2];
    else if (docType === 'Test' && match[2]) docId = match[2];
    else if (docType === 'StorySupplement' && match[3]) docId = match[3];
    else if (docType === 'PRD-or-Other' && match[2]) docId = match[2];

    // 提取 storyId（如有）
    let storyId = null;
    if (['Task', 'Coding', 'CR', 'Test', 'StorySupplement'].includes(docType)) {
      storyId = match[1];
    } else if (docType === 'Story') {
      storyId = match[1];
    }

    plan.push({
      source: relPath,
      target: targetRel,
      docId,
      docType,
      storyId,
    });
  }

  return { plan, skipped, allFiles };
}

// ========== 执行迁移 ==========

function executeMigration(rootPath, plan, date, author) {
  const results = { success: 0, failed: 0, errors: [] };

  for (const item of plan) {
    const sourcePath = path.join(rootPath, item.source);
    const targetPath = path.join(rootPath, item.target);

    try {
      // 1. 创建目标目录
      fs.mkdirSync(path.dirname(targetPath), { recursive: true });

      // 2. 读取源文件
      const content = fs.readFileSync(sourcePath, 'utf-8');

      // 3. 写入目标文件（旧版本不删）
      fs.writeFileSync(targetPath, content, 'utf-8');

      // 4. 追加 ChangeLog（如果目标文件已存在则追加；否则创建）
      const changelogDir = path.join(rootPath, `ae-sdd-doc/iterations/${date}/${item.docType}/ChangeLog`);
      fs.mkdirSync(changelogDir, { recursive: true });
      const changelogPath = path.join(changelogDir, `${item.docId}-changelog.md`);
      const changelogEntry = `| v1.0 | ${date} | ${author} | 迁移自旧路径 ${item.source} | migrate-docs.mjs |\n`;

      if (fs.existsSync(changelogPath)) {
        const existing = fs.readFileSync(changelogPath, 'utf-8');
        if (!existing.includes('迁移自旧路径 ' + item.source)) {
          fs.appendFileSync(changelogPath, changelogEntry, 'utf-8');
        }
      } else {
        const header = `# ChangeLog - ${item.docId}\n\n| 版本 | 日期 | 修改人 | 修改项 | 改动来源 |\n|------|------|--------|--------|---------|\n`;
        fs.writeFileSync(changelogPath, header + changelogEntry, 'utf-8');
      }

      results.success++;
    } catch (e) {
      results.failed++;
      results.errors.push({ source: item.source, error: e.message });
    }
  }

  return results;
}

// ========== 报告生成 ==========

function generateReport(plan, skipped, results, args) {
  const lines = [];
  lines.push(`# Migration Report - ${args.target} - ${args.date}`);
  lines.push('');
  lines.push(`## 模式: ${args.dryRun ? 'DRY-RUN（不执行）' : 'EXECUTE（已执行）'}`);
  lines.push('');

  // 按 docType 统计
  const byType = {};
  for (const item of plan) {
    byType[item.docType] = (byType[item.docType] || 0) + 1;
  }

  lines.push('## 扫描结果（按 docType）');
  lines.push('');
  lines.push('| docType | 文件数 |');
  lines.push('|---------|--------|');
  for (const [type, count] of Object.entries(byType)) {
    lines.push(`| ${type} | ${count} |`);
  }
  lines.push(`| **合计** | **${plan.length}** |`);
  if (skipped.length > 0) {
    lines.push('');
    lines.push(`> ⚠️ 跳过 ${skipped.length} 个文件（未匹配任何规则）：`);
    for (const s of skipped.slice(0, 20)) {
      lines.push(`> - ${s}`);
    }
    if (skipped.length > 20) {
      lines.push(`> - ...及其他 ${skipped.length - 20} 个`);
    }
  }
  lines.push('');

  lines.push('## 迁移计划');
  lines.push('');
  lines.push('| # | 源路径 | 目标路径 | doc_id | doc_type |');
  lines.push('|---|--------|---------|--------|----------|');
  plan.forEach((item, i) => {
    lines.push(`| ${i + 1} | \`${item.source}\` | \`${item.target}\` | ${item.docId} | ${item.docType} |`);
  });
  lines.push('');

  if (results) {
    lines.push('## 执行结果');
    lines.push('');
    lines.push(`- ✅ 成功: ${results.success}`);
    lines.push(`- ❌ 失败: ${results.failed}`);
    if (results.errors.length > 0) {
      lines.push('');
      lines.push('### 错误清单');
      for (const err of results.errors) {
        lines.push(`- \`${err.source}\`: ${err.error}`);
      }
    }
    lines.push('');
  }

  lines.push('## 注意事项');
  lines.push('');
  lines.push('- 🔴 旧目录（`design/`、`.ae-task/`、`.ae-plan/`、`.spec/iterations/`）**保留不删除**');
  lines.push('- ChangeLog 初始行标记 "迁移自旧路径 ..."');
  lines.push('- 所有迁移文件默认 v1.0');
  lines.push('- 如需回滚：直接删除 `ae-sdd-doc/iterations/{date}/` 下的对应文件即可');
  lines.push('');

  return lines.join('\n');
}

// ========== 主流程 ==========

function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.help) {
    showHelp();
    process.exit(0);
  }

  if (!args.target) {
    console.error('❌ 错误：必须指定 --target <path>');
    showHelp();
    process.exit(1);
  }

  if (!fs.existsSync(args.target)) {
    console.error(`❌ 错误：目标路径不存在: ${args.target}`);
    process.exit(1);
  }

  console.log(`🔍 扫描工程根: ${args.target}`);
  console.log(`📅 迭代日期: ${args.date}`);
  console.log(`⚙️  模式: ${args.dryRun ? 'DRY-RUN（不执行）' : 'EXECUTE（已执行）'}`);
  console.log('');

  const { plan, skipped } = buildMigrationPlan(args.target, args.date, args.author);

  console.log(`📊 扫描到 ${plan.length} 个待迁移文件，${skipped.length} 个跳过`);
  console.log('');

  let results = null;
  if (args.execute) {
    console.log('🚀 开始执行迁移...');
    results = executeMigration(args.target, plan, args.date, args.author);
    console.log(`✅ 成功 ${results.success}，❌ 失败 ${results.failed}`);
    console.log('');
  } else {
    console.log('💡 当前为 DRY-RUN 模式，未修改任何文件');
    console.log('   加上 --execute 参数才会真正执行迁移');
    console.log('');
  }

  // 输出报告
  const report = generateReport(plan, skipped, results, args);
  const reportPath = path.join(args.target, `ae-sdd-doc/iterations/${args.date}/_migration-report-${args.date}.md`);
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, report, 'utf-8');
  console.log(`📄 报告已生成: ${reportPath}`);
  console.log('');

  // 控制台输出摘要
  console.log('--- 报告摘要 ---');
  console.log(report);
}

main();
