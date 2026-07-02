import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { installSkills, parseInstallArgs } from '../src/install-skills.js';

// Tests run from dist/test, so the package root is two levels up.
const PKG_VERSION = JSON.parse(
  await readFile(new URL('../../package.json', import.meta.url), 'utf8'),
).version as string;

test('installSkills writes skill files and an npx MCP entry by default', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'nubase-skill-'));
  const home = await mkdtemp(path.join(os.tmpdir(), 'nubase-home-'));
  try {
    const files = await installSkills({ target: 'both', projectDir: dir, homeDir: home });
    // 2 skills (claude + codex) + .mcp.json + .gitignore — no per-project bridge copy.
    assert.equal(files.length, 4);
    const claude = await readFile(path.join(home, '.claude', 'skills', 'nubase', 'SKILL.md'), 'utf8');
    const codex = await readFile(path.join(home, '.codex', 'skills', 'nubase', 'references', 'memory.md'), 'utf8');
    const security = await readFile(path.join(home, '.claude', 'skills', 'nubase', 'references', 'security.md'), 'utf8');
    const mcpConfig = JSON.parse(await readFile(path.join(dir, '.mcp.json'), 'utf8'));
    const gitignore = await readFile(path.join(dir, '.gitignore'), 'utf8');
    assert.match(claude, /Nubase Core Skill/);
    assert.match(codex, /memory_context/);
    assert.match(security, /service_role/);
    assert.deepEqual(mcpConfig.mcpServers.nubase, {
      type: 'stdio',
      command: 'npx',
      args: ['-y', `nubase_cli@${PKG_VERSION}`],
      env: {
        NUBASE_AGENT_ID: 'claude-code',
        NUBASE_CONFIG: path.join(dir, '.nubase', 'config.json'),
        NUBASE_ALLOW_SQL_EXECUTE: 'true',
        NUBASE_ALLOW_ADMIN_WRITE: 'true',
      },
    });
    // npx delivery must not copy the bridge into the project.
    await assert.rejects(stat(path.join(dir, '.nubase', 'mcp-bridge', 'dist', 'src', 'index.js')));
    assert.match(gitignore, /^\.nubase\/$/m);
  } finally {
    await rm(dir, { recursive: true, force: true });
    await rm(home, { recursive: true, force: true });
  }
});

test('installSkills with local delivery copies the bridge into the project', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'nubase-skill-'));
  const home = await mkdtemp(path.join(os.tmpdir(), 'nubase-home-'));
  try {
    const files = await installSkills({ target: 'both', projectDir: dir, homeDir: home, mcpDelivery: 'local' });
    assert.equal(files.length, 5);
    const mcpEntrypoint = path.join(dir, '.nubase', 'mcp-bridge', 'dist', 'src', 'index.js');
    const mcpConfig = JSON.parse(await readFile(path.join(dir, '.mcp.json'), 'utf8'));
    assert.deepEqual(mcpConfig.mcpServers.nubase, {
      type: 'stdio',
      command: 'node',
      args: [mcpEntrypoint],
      env: {
        NUBASE_AGENT_ID: 'claude-code',
        NUBASE_CONFIG: path.join(dir, '.nubase', 'config.json'),
        NUBASE_ALLOW_SQL_EXECUTE: 'true',
        NUBASE_ALLOW_ADMIN_WRITE: 'true',
      },
    });
    assert.equal((await stat(mcpEntrypoint)).isFile(), true);
  } finally {
    await rm(dir, { recursive: true, force: true });
    await rm(home, { recursive: true, force: true });
  }
});

test('installSkills preserves existing Claude MCP servers', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'nubase-skill-'));
  try {
    await writeFile(path.join(dir, '.mcp.json'), JSON.stringify({
      mcpServers: {
        existing: {
          command: 'node',
          args: ['server.js'],
        },
      },
    }));

    await installSkills({ target: 'claude', projectDir: dir });
    const mcpConfig = JSON.parse(await readFile(path.join(dir, '.mcp.json'), 'utf8'));
    assert.deepEqual(mcpConfig.mcpServers.existing, {
      command: 'node',
      args: ['server.js'],
    });
    assert.equal(mcpConfig.mcpServers.nubase.command, 'npx');
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('installSkills can write project-scoped skills and Codex MCP config', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'nubase-skill-'));
  try {
    await installSkills({ target: 'both', projectDir: dir, skillsScope: 'project', mcp: 'both' });
    const claude = await readFile(path.join(dir, '.claude', 'skills', 'nubase', 'SKILL.md'), 'utf8');
    const codex = await readFile(path.join(dir, '.codex', 'skills', 'nubase', 'SKILL.md'), 'utf8');
    const codexConfig = await readFile(path.join(dir, '.codex', 'config.toml'), 'utf8');
    assert.match(claude, /Nubase Core Skill/);
    assert.match(codex, /Nubase Core Skill/);
    assert.match(codexConfig, /\[mcp_servers\.nubase\]/);
    assert.match(codexConfig, /command = "npx"/);
    assert.match(codexConfig, new RegExp(`nubase_cli@${PKG_VERSION.replace(/\./g, '\\.')}`));
    assert.match(codexConfig, /NUBASE_CONFIG = ".+\.nubase\/config\.json"/);
    assert.match(codexConfig, /NUBASE_ALLOW_SQL_EXECUTE = "true"/);
    assert.match(codexConfig, /NUBASE_ALLOW_ADMIN_WRITE = "true"/);
    assert.doesNotMatch(codexConfig, /NUBASE_ALLOW_DANGEROUS_SQL/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('installSkills can opt out of write permissions and opt into dangerous SQL', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'nubase-skill-'));
  const home = await mkdtemp(path.join(os.tmpdir(), 'nubase-home-'));
  try {
    await installSkills({
      target: 'claude',
      projectDir: dir,
      homeDir: home,
      allowSqlExecute: false,
      allowAdminWrite: false,
      allowDangerousSql: true,
    });
    const mcpConfig = JSON.parse(await readFile(path.join(dir, '.mcp.json'), 'utf8'));
    assert.deepEqual(mcpConfig.mcpServers.nubase.env, {
      NUBASE_AGENT_ID: 'claude-code',
      NUBASE_CONFIG: path.join(dir, '.nubase', 'config.json'),
      NUBASE_ALLOW_DANGEROUS_SQL: 'true',
    });
  } finally {
    await rm(dir, { recursive: true, force: true });
    await rm(home, { recursive: true, force: true });
  }
});

test('parseInstallArgs parses target and project dir', () => {
  const parsed = parseInstallArgs(['--target', 'claude', '--project-dir', '/tmp/example']);
  assert.equal(parsed.target, 'claude');
  assert.equal(parsed.projectDir, '/tmp/example');
  assert.equal(parsed.authorize, true);
  assert.equal(parsed.skillsScope, 'user');
  assert.equal(parsed.mcp, 'claude');
  assert.equal(parsed.mcpDelivery, 'npx');
  assert.equal(parsed.allowSqlExecute, true);
  assert.equal(parsed.allowAdminWrite, true);
  assert.equal(parsed.allowDangerousSql, false);
  assert.equal(parsed.configPath, path.join('/tmp/example', '.nubase', 'config.json'));
  assert.deepEqual(parsed.authArgs, ['--prompt-only', '--config', path.join('/tmp/example', '.nubase', 'config.json')]);
});

test('parseInstallArgs passes authorization options through', () => {
  const parsed = parseInstallArgs([
    '--target',
    'codex',
    '--no-authorize',
    '--studio-url',
    'https://studio.example.com',
    '--nubase-url',
    'https://api.example.com',
    '--agent-id',
    'codex',
  ]);
  assert.equal(parsed.authorize, false);
  assert.deepEqual(parsed.authArgs, [
    '--prompt-only',
    '--studio-url',
    'https://studio.example.com',
    '--nubase-url',
    'https://api.example.com',
    '--agent-id',
    'codex',
    '--config',
    path.join(process.cwd(), '.nubase', 'config.json'),
  ]);
});

test('parseInstallArgs can skip MCP config registration', () => {
  const parsed = parseInstallArgs(['--no-mcp-config', '--no-skills', '--skills-scope', 'project', '--mcp', 'none', '--mcp-delivery', 'local', '--no-sql-execute', '--no-admin-write', '--allow-dangerous-sql']);
  assert.equal(parsed.mcp, 'none');
  assert.equal(parsed.skills, false);
  assert.equal(parsed.skillsScope, 'project');
  assert.equal(parsed.mcpDelivery, 'local');
  assert.equal(parsed.allowSqlExecute, false);
  assert.equal(parsed.allowAdminWrite, false);
  assert.equal(parsed.allowDangerousSql, true);
});
