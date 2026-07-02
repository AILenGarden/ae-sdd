import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import http from 'node:http';
import { defaultConfigPath, saveStoredAuthConfig, type StoredAuthConfig } from './auth-config.js';
import { DEFAULT_NUBASE_URL } from './config.js';

const DEFAULT_STUDIO_URL = 'https://nubase.ai/studio';

export interface AuthorizeOptions {
  nubaseUrl: string;
  studioUrl: string;
  agentId?: string;
  openBrowser: boolean;
  timeoutMs: number;
  promptOnly: boolean;
  configPath: string;
}

interface AuthorizeCallbackPayload {
  state?: string;
  nubaseUrl?: string;
  projectKey?: string;
  projectRef?: string;
  projectName?: string;
  anonKey?: string;
  userId?: string;
  userEmail?: string;
}

export function parseAuthorizeArgs(argv: string[], env: NodeJS.ProcessEnv = process.env): AuthorizeOptions {
  const options: AuthorizeOptions = {
    nubaseUrl: stripTrailingSlash(env.NUBASE_URL || DEFAULT_NUBASE_URL),
    studioUrl: stripTrailingSlash(env.NUBASE_STUDIO_URL || DEFAULT_STUDIO_URL),
    agentId: blankToUndefined(env.NUBASE_AGENT_ID),
    openBrowser: true,
    timeoutMs: 5 * 60 * 1000,
    promptOnly: false,
    configPath: defaultConfigPath(env),
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--nubase-url') {
      options.nubaseUrl = stripTrailingSlash(requiredValue(argv, ++i, arg));
    } else if (arg === '--studio-url') {
      options.studioUrl = stripTrailingSlash(requiredValue(argv, ++i, arg));
    } else if (arg === '--agent-id') {
      options.agentId = requiredValue(argv, ++i, arg);
    } else if (arg === '--config') {
      options.configPath = requiredValue(argv, ++i, arg);
    } else if (arg === '--timeout-seconds') {
      const seconds = Number(requiredValue(argv, ++i, arg));
      if (!Number.isFinite(seconds) || seconds <= 0) {
        throw new Error('--timeout-seconds must be a positive number');
      }
      options.timeoutMs = seconds * 1000;
    } else if (arg === '--no-open') {
      options.openBrowser = false;
    } else if (arg === '--prompt-only') {
      options.openBrowser = false;
      options.promptOnly = true;
    } else {
      throw new Error(`Unknown authorize option: ${arg}`);
    }
  }

  return options;
}

export async function authorize(options: AuthorizeOptions): Promise<StoredAuthConfig> {
  return startAuthorization(options);
}

export function buildAuthorizeUrl({
  studioUrl,
  nubaseUrl,
  callbackUrl,
  sessionId,
  state,
  agentId,
}: {
  studioUrl: string;
  nubaseUrl: string;
  callbackUrl: string;
  sessionId: string;
  state: string;
  agentId?: string;
}) {
  const authorizeUrl = new URL(`${stripTrailingSlash(studioUrl)}/cli/authorize`);
  authorizeUrl.searchParams.set('callback', callbackUrl);
  authorizeUrl.searchParams.set('session_id', sessionId);
  authorizeUrl.searchParams.set('state', state);
  authorizeUrl.searchParams.set('nubase_url', stripTrailingSlash(nubaseUrl));
  if (agentId) authorizeUrl.searchParams.set('agent_id', agentId);
  return authorizeUrl;
}

async function startAuthorization(options: AuthorizeOptions): Promise<StoredAuthConfig> {
  const sessionId = crypto.randomUUID();
  const state = crypto.randomBytes(24).toString('base64url');
  const server = http.createServer();

  const result = await new Promise<StoredAuthConfig>((resolve, reject) => {
    const timeout = setTimeout(() => {
      cleanup();
      reject(new Error('Timed out waiting for browser authorization.'));
    }, options.timeoutMs);

    const cleanup = () => {
      clearTimeout(timeout);
      server.close();
    };

    server.on('request', async (req, res) => {
      const origin = callbackOrigin(server);
      if (!origin) {
        sendHtml(res, 500, 'Nubase CLI authorization failed', 'The local callback server was not ready.');
        return;
      }

      try {
        if (req.method === 'OPTIONS' && req.url === '/callback') {
          sendCorsNoContent(res);
          return;
        }

        if (req.method === 'GET' && req.url?.startsWith('/callback')) {
          const url = new URL(req.url, origin);
          if (url.searchParams.get('state') !== state) {
            sendHtml(res, 400, 'Nubase CLI authorization failed', 'The authorization state did not match.');
            return;
          }
          sendHtml(res, 200, 'Nubase CLI is waiting', 'Return to the Studio tab to finish authorization.');
          return;
        }

        if (req.method === 'POST' && req.url === '/callback') {
          const payload = await readJson(req);
          const config = validateCallbackPayload(payload, state, options.nubaseUrl);
          const saved = await saveStoredAuthConfig(config, options.configPath);
          sendJson(res, 200, { ok: true });
          cleanup();
          resolve(saved);
          return;
        }

        sendJson(res, 404, { error: 'not_found' });
      } catch (err) {
        sendJson(res, 400, { error: (err as Error).message });
      }
    });

    server.once('error', (err) => {
      cleanup();
      reject(err);
    });

    server.listen(0, '127.0.0.1', () => {
      const callbackUrl = `${callbackOrigin(server)}/callback`;
      const authorizeUrl = buildAuthorizeUrl({
        studioUrl: options.studioUrl,
        nubaseUrl: options.nubaseUrl,
        callbackUrl,
        sessionId,
        state,
        agentId: options.agentId,
      });

      console.error('Authorize Nubase CLI for this workspace:');
      console.error(authorizeUrl.toString());
      console.error('');
      console.error(`Waiting for browser authorization session ${sessionId} on ${callbackUrl}`);

      if (options.openBrowser && !options.promptOnly) {
        openBrowser(authorizeUrl.toString());
      }
    });
  });

  return result;
}

function validateCallbackPayload(
  payload: AuthorizeCallbackPayload,
  state: string,
  defaultNubaseUrl: string
): Omit<StoredAuthConfig, 'savedAt'> {
  if (payload.state !== state) {
    throw new Error('Invalid authorization state.');
  }
  if (typeof payload.projectKey !== 'string' || !payload.projectKey.trim()) {
    throw new Error('Missing project key in authorization callback.');
  }
  return {
    nubaseUrl: stripTrailingSlash(payload.nubaseUrl || defaultNubaseUrl),
    projectKey: payload.projectKey.trim(),
    projectRef: blankToUndefined(payload.projectRef),
    projectName: blankToUndefined(payload.projectName),
    anonKey: blankToUndefined(payload.anonKey),
    userId: blankToUndefined(payload.userId),
    userEmail: blankToUndefined(payload.userEmail),
  };
}

function callbackOrigin(server: http.Server) {
  const address = server.address();
  if (!address || typeof address === 'string') return null;
  return `http://127.0.0.1:${address.port}`;
}

async function readJson(req: http.IncomingMessage) {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8')) as AuthorizeCallbackPayload;
}

function sendJson(res: http.ServerResponse, status: number, body: unknown) {
  res.writeHead(status, {
    'Content-Type': 'application/json',
    ...corsHeaders(),
  });
  res.end(JSON.stringify(body));
}

function sendCorsNoContent(res: http.ServerResponse) {
  res.writeHead(204, corsHeaders());
  res.end();
}

function corsHeaders() {
  return {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET,POST,OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
    'Access-Control-Allow-Private-Network': 'true',
    'Access-Control-Max-Age': '600',
  };
}

function sendHtml(res: http.ServerResponse, status: number, title: string, message: string) {
  res.writeHead(status, { 'Content-Type': 'text/html; charset=utf-8', ...corsHeaders() });
  res.end(`<!doctype html><meta charset="utf-8"><title>${escapeHtml(title)}</title><body style="font-family: system-ui, sans-serif; padding: 32px;"><h1>${escapeHtml(title)}</h1><p>${escapeHtml(message)}</p></body>`);
}

function openBrowser(url: string) {
  const platform = process.platform;
  const command = platform === 'darwin' ? 'open' : platform === 'win32' ? 'cmd' : 'xdg-open';
  const args = platform === 'win32' ? ['/c', 'start', '', url] : [url];
  const child = spawn(command, args, { detached: true, stdio: 'ignore' });
  child.on('error', () => undefined);
  child.unref();
}

function requiredValue(argv: string[], index: number, flag: string) {
  const value = argv[index];
  if (!value) throw new Error(`${flag} requires a value`);
  return value;
}

function stripTrailingSlash(value: string) {
  return value.replace(/\/+$/, '');
}

function blankToUndefined(value: unknown) {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
