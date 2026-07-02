import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { projectConfigPath } from './auth-config.js';

export type SkillTarget = 'claude' | 'codex' | 'both';
export type SkillInstallScope = 'user' | 'project';
export type McpInstallTarget = 'none' | 'claude' | 'codex' | 'both';
// How the MCP bridge code is delivered:
//   'npx'   – shared via the npm cache (npx -y nubase_cli@<version>); the project
//             only keeps config.json + .mcp.json. Reusable across projects, portable.
//   'local' – copy the bundled dist into <projectDir>/.nubase/mcp-bridge (hermetic,
//             version-pinned per project, but duplicated and non-portable).
export type McpDelivery = 'npx' | 'local';

export interface InstallSkillsOptions {
  target: SkillTarget;
  projectDir: string;
  authorize?: boolean;
  authArgs?: string[];
  skills?: boolean;
  skillsScope?: SkillInstallScope;
  mcp?: McpInstallTarget;
  mcpDelivery?: McpDelivery;
  // Permission env flags written into the MCP config. Reads are always allowed;
  // these gate write/execute tools. SQL execute and admin write default ON;
  // dangerous SQL (DROP/TRUNCATE/...) defaults OFF.
  allowSqlExecute?: boolean;
  allowAdminWrite?: boolean;
  allowDangerousSql?: boolean;
  configPath?: string;
  homeDir?: string;
}

interface ProjectMcpConfig {
  mcpServers?: Record<string, unknown>;
  [key: string]: unknown;
}

interface McpCommand {
  command: string;
  args: string[];
  entrypoint: string;
}

export async function installSkills(options: InstallSkillsOptions) {
  const skillDir = bundledSkillDir();
  const targets = options.target === 'both' ? ['claude', 'codex'] as const : [options.target] as const;
  const installed: string[] = [];
  const skillsScope = options.skillsScope ?? 'user';
  const configPath = path.resolve(options.configPath ?? projectConfigPath(options.projectDir));
  const homeDir = options.homeDir ?? os.homedir();

  if (options.skills !== false) {
    for (const target of targets) {
      const destDir = skillDestDir(target, skillsScope, options.projectDir, homeDir);
      await mkdir(path.dirname(destDir), { recursive: true });
      await cp(skillDir, destDir, { recursive: true, force: true });
      installed.push(path.join(destDir, 'SKILL.md'));
    }
  }

  const mcpTargets = resolveMcpTargets(options.mcp ?? 'claude', targets);
  const mcpDelivery: McpDelivery = options.mcpDelivery ?? 'npx';
  let mcpCommand: McpCommand | null = null;
  if (mcpTargets.length > 0) {
    if (mcpDelivery === 'local') {
      mcpCommand = await installProjectMcpBridge(options.projectDir);
      installed.push(mcpCommand.entrypoint);
    } else {
      mcpCommand = await npxMcpCommand();
    }
  }
  const permissionEnv = buildPermissionEnv(options);
  if (mcpTargets.includes('claude')) {
    installed.push(await installClaudeMcpConfig(options.projectDir, configPath, mcpCommand, permissionEnv));
  }
  if (mcpTargets.includes('codex')) {
    installed.push(await installCodexMcpConfig(options.projectDir, configPath, mcpCommand, permissionEnv));
  }
  installed.push(await ensureProjectGitignore(options.projectDir));

  return installed;
}

export function parseInstallArgs(argv: string[]) {
  let target: SkillTarget = 'both';
  let projectDir = process.cwd();
  let authorize = true;
  let skills = true;
  let skillsScope: SkillInstallScope = 'user';
  let mcp: McpInstallTarget = 'claude';
  let mcpDelivery: McpDelivery = 'npx';
  let allowSqlExecute = true;
  let allowAdminWrite = true;
  let allowDangerousSql = false;
  let configPath: string | undefined;
  const authArgs: string[] = ['--prompt-only'];
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--target') {
      const value = argv[++i];
      if (value !== 'claude' && value !== 'codex' && value !== 'both') {
        throw new Error('--target must be claude, codex, or both');
      }
      target = value;
    } else if (arg === '--project-dir') {
      const value = argv[++i];
      if (!value) throw new Error('--project-dir requires a value');
      projectDir = path.resolve(value);
    } else if (arg === '--no-authorize') {
      authorize = false;
    } else if (arg === '--no-skills') {
      skills = false;
    } else if (arg === '--no-mcp-config') {
      mcp = 'none';
    } else if (arg === '--no-mcp') {
      mcp = 'none';
    } else if (arg === '--mcp') {
      const value = argv[++i];
      if (value !== 'none' && value !== 'claude' && value !== 'codex' && value !== 'both') {
        throw new Error('--mcp must be none, claude, codex, or both');
      }
      mcp = value;
    } else if (arg === '--skills-scope') {
      const value = argv[++i];
      if (value !== 'user' && value !== 'project') {
        throw new Error('--skills-scope must be user or project');
      }
      skillsScope = value;
    } else if (arg === '--mcp-delivery') {
      const value = argv[++i];
      if (value !== 'npx' && value !== 'local') {
        throw new Error('--mcp-delivery must be npx or local');
      }
      mcpDelivery = value;
    } else if (arg === '--no-sql-execute') {
      allowSqlExecute = false;
    } else if (arg === '--no-admin-write') {
      allowAdminWrite = false;
    } else if (arg === '--allow-dangerous-sql') {
      allowDangerousSql = true;
    } else if (arg === '--config') {
      const value = argv[++i];
      if (!value) throw new Error('--config requires a value');
      configPath = path.resolve(projectDir, value);
    } else if (arg === '--studio-url' || arg === '--nubase-url' || arg === '--agent-id' || arg === '--timeout-seconds') {
      const value = argv[++i];
      if (!value) throw new Error(`${arg} requires a value`);
      authArgs.push(arg, value);
    } else {
      throw new Error(`Unknown install-skills option: ${arg}`);
    }
  }

  configPath = configPath ?? projectConfigPath(projectDir);
  authArgs.push('--config', configPath);
  return {
    target,
    projectDir,
    authorize,
    authArgs,
    skills,
    skillsScope,
    mcp,
    mcpDelivery,
    allowSqlExecute,
    allowAdminWrite,
    allowDangerousSql,
    configPath,
  };
}

function bundledSkillDir() {
  return path.join(bundledPackageRoot(), 'skills', 'nubase');
}

function bundledPackageRoot() {
  const here = path.dirname(fileURLToPath(import.meta.url));
  return path.resolve(here, '..', '..');
}

function skillDestDir(target: Exclude<SkillTarget, 'both'>, scope: SkillInstallScope, projectDir: string, homeDir: string) {
  if (scope === 'project') {
    return target === 'claude'
      ? path.join(projectDir, '.claude', 'skills', 'nubase')
      : path.join(projectDir, '.codex', 'skills', 'nubase');
  }
  return target === 'claude'
    ? path.join(homeDir, '.claude', 'skills', 'nubase')
    : path.join(homeDir, '.codex', 'skills', 'nubase');
}

function resolveMcpTargets(mcp: McpInstallTarget, skillTargets: readonly Exclude<SkillTarget, 'both'>[]) {
  if (mcp === 'none') return [];
  const requested = mcp === 'both' ? ['claude', 'codex'] as const : [mcp] as const;
  return requested.filter((target) => skillTargets.includes(target));
}

async function npxMcpCommand(): Promise<McpCommand> {
  const spec = `nubase_cli@${await bundledPackageVersion()}`;
  return { command: 'npx', args: ['-y', spec], entrypoint: spec };
}

async function bundledPackageVersion(): Promise<string> {
  try {
    const raw = await readFile(path.join(bundledPackageRoot(), 'package.json'), 'utf8');
    const version = (JSON.parse(raw) as { version?: unknown }).version;
    return typeof version === 'string' && version.trim() ? version.trim() : 'latest';
  } catch {
    return 'latest';
  }
}

async function installProjectMcpBridge(projectDir: string): Promise<McpCommand> {
  const packageRoot = bundledPackageRoot();
  const destRoot = path.join(projectDir, '.nubase', 'mcp-bridge');
  await rm(destRoot, { recursive: true, force: true });
  await mkdir(destRoot, { recursive: true, mode: 0o700 });
  await cp(path.join(packageRoot, 'dist', 'src'), path.join(destRoot, 'dist', 'src'), { recursive: true, force: true });
  await cp(path.join(packageRoot, 'skills'), path.join(destRoot, 'skills'), { recursive: true, force: true });
  await cp(path.join(packageRoot, 'package.json'), path.join(destRoot, 'package.json'), { force: true });
  const entrypoint = path.join(destRoot, 'dist', 'src', 'index.js');
  return {
    command: 'node',
    args: [entrypoint],
    entrypoint,
  };
}

// Defaults: SQL execute + admin write ON, dangerous SQL OFF. Reads never need a flag.
function buildPermissionEnv(options: InstallSkillsOptions): Record<string, string> {
  const env: Record<string, string> = {};
  if (options.allowSqlExecute ?? true) env.NUBASE_ALLOW_SQL_EXECUTE = 'true';
  if (options.allowAdminWrite ?? true) env.NUBASE_ALLOW_ADMIN_WRITE = 'true';
  if (options.allowDangerousSql ?? false) env.NUBASE_ALLOW_DANGEROUS_SQL = 'true';
  return env;
}

async function installClaudeMcpConfig(
  projectDir: string,
  nubaseConfigPath: string,
  mcpCommand: McpCommand | null,
  permissionEnv: Record<string, string>,
) {
  const mcpConfigPath = path.join(projectDir, '.mcp.json');
  const config = await readProjectMcpConfig(mcpConfigPath);
  config.mcpServers = {
    ...(config.mcpServers ?? {}),
    nubase: {
      type: 'stdio',
      command: mcpCommand?.command ?? 'npx',
      args: mcpCommand?.args ?? ['-y', 'nubase_cli@latest'],
      env: {
        NUBASE_AGENT_ID: 'claude-code',
        NUBASE_CONFIG: nubaseConfigPath,
        ...permissionEnv,
      },
    },
  };
  await writeFile(mcpConfigPath, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
  return mcpConfigPath;
}

async function installCodexMcpConfig(
  projectDir: string,
  nubaseConfigPath: string,
  mcpCommand: McpCommand | null,
  permissionEnv: Record<string, string>,
) {
  const configPath = path.join(projectDir, '.codex', 'config.toml');
  await mkdir(path.dirname(configPath), { recursive: true });
  const existing = await readTextIfExists(configPath);
  const block = codexMcpBlock(nubaseConfigPath, mcpCommand, permissionEnv);
  const next = upsertCodexMcpBlock(existing, block);
  await writeFile(configPath, next, 'utf8');
  return configPath;
}

async function ensureProjectGitignore(projectDir: string) {
  const gitignorePath = path.join(projectDir, '.gitignore');
  const existing = await readTextIfExists(gitignorePath);
  const lines = existing.split(/\r?\n/);
  if (!lines.includes('.nubase/')) {
    const next = `${existing.trimEnd()}${existing.trimEnd() ? '\n' : ''}.nubase/\n`;
    await writeFile(gitignorePath, next, 'utf8');
  }
  return gitignorePath;
}

async function readTextIfExists(filePath: string) {
  try {
    return await readFile(filePath, 'utf8');
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') return '';
    throw err;
  }
}

function codexMcpBlock(configPath: string, mcpCommand: McpCommand | null, permissionEnv: Record<string, string>) {
  const command = mcpCommand?.command ?? 'npx';
  const args = mcpCommand?.args ?? ['-y', 'nubase_cli@latest'];
  const permissionLines = Object.entries(permissionEnv).map(
    ([key, value]) => `${key} = "${escapeTomlString(value)}"`,
  );
  return [
    '[mcp_servers.nubase]',
    'type = "stdio"',
    `command = "${escapeTomlString(command)}"`,
    `args = [${args.map((arg) => `"${escapeTomlString(arg)}"`).join(', ')}]`,
    'startup_timeout_sec = 30',
    '',
    '[mcp_servers.nubase.env]',
    'NUBASE_AGENT_ID = "codex"',
    `NUBASE_CONFIG = "${escapeTomlString(configPath)}"`,
    ...permissionLines,
    '',
  ].join('\n');
}

function upsertCodexMcpBlock(existing: string, block: string) {
  const pattern = /(?:^|\n)\[mcp_servers\.nubase\][\s\S]*?(?=\n\[mcp_servers\.(?!nubase(?:\.env)?\b)|\n\[[^\]]+\]|\s*$)/;
  const trimmedBlock = `\n${block.trimEnd()}\n`;
  if (pattern.test(existing)) {
    return existing.replace(pattern, trimmedBlock).replace(/^\n/, '');
  }
  const prefix = existing.trimEnd();
  return `${prefix}${prefix ? '\n\n' : ''}${block}`;
}

function escapeTomlString(value: string) {
  return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

async function readProjectMcpConfig(configPath: string): Promise<ProjectMcpConfig> {
  try {
    const raw = await readFile(configPath, 'utf8');
    const parsed = JSON.parse(raw) as ProjectMcpConfig;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('.mcp.json must contain a JSON object');
    }
    if (parsed.mcpServers !== undefined && (!parsed.mcpServers || typeof parsed.mcpServers !== 'object' || Array.isArray(parsed.mcpServers))) {
      throw new Error('.mcp.json mcpServers must be a JSON object');
    }
    return parsed;
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') return {};
    if (err instanceof SyntaxError) {
      throw new Error(`Could not parse ${configPath}: ${err.message}`);
    }
    throw err;
  }
}
