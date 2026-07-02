import { mkdir, readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

export interface StoredAuthConfig {
  nubaseUrl: string;
  projectKey: string;
  projectRef?: string;
  projectName?: string;
  // The anon/authenticated key for client apps (projectKey is the service_role key).
  anonKey?: string;
  userId?: string;
  userEmail?: string;
  savedAt: string;
}

export function defaultConfigPath(env: NodeJS.ProcessEnv = process.env) {
  if (env.NUBASE_CONFIG) return env.NUBASE_CONFIG;
  return projectConfigPath();
}

export function projectConfigPath(projectDir = process.cwd()) {
  return path.join(projectDir, '.nubase', 'config.json');
}

export function legacyConfigPath() {
  return path.join(os.homedir(), '.nubase', 'config.json');
}

export async function loadStoredAuthConfig(configPath = defaultConfigPath()) {
  try {
    const raw = await readFile(configPath, 'utf8');
    const parsed = JSON.parse(raw) as Partial<StoredAuthConfig>;
    if (typeof parsed.nubaseUrl !== 'string' || typeof parsed.projectKey !== 'string') {
      return null;
    }
    return {
      nubaseUrl: stripTrailingSlash(parsed.nubaseUrl),
      projectKey: parsed.projectKey,
      projectRef: blankToUndefined(parsed.projectRef),
      projectName: blankToUndefined(parsed.projectName),
      anonKey: blankToUndefined(parsed.anonKey),
      userId: blankToUndefined(parsed.userId),
      userEmail: blankToUndefined(parsed.userEmail),
      savedAt: parsed.savedAt || '',
    } satisfies StoredAuthConfig;
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw err;
  }
}

export async function saveStoredAuthConfig(config: Omit<StoredAuthConfig, 'savedAt'>, configPath = defaultConfigPath()) {
  const body: StoredAuthConfig = {
    ...config,
    nubaseUrl: stripTrailingSlash(config.nubaseUrl),
    savedAt: new Date().toISOString(),
  };
  await mkdir(path.dirname(configPath), { recursive: true, mode: 0o700 });
  await writeFile(configPath, `${JSON.stringify(body, null, 2)}\n`, { mode: 0o600 });
  return body;
}

function stripTrailingSlash(value: string) {
  return value.replace(/\/+$/, '');
}

function blankToUndefined(value: unknown) {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}
