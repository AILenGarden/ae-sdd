import http from 'node:http';
import { spawn } from 'node:child_process';
import { randomBytes, createHash } from 'node:crypto';
import { readFileSync, rmSync } from 'node:fs';
import { dirname, join, posix, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const aliasPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const accountPattern = /^[A-Za-z_][A-Za-z0-9._-]{0,63}$/;
const fingerprint = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
// Platform facts are read per call so callers (and tests) can pin the target platform.
const isWindows = () => process.platform === 'win32';
const isMac = () => process.platform === 'darwin';
const platformTag = () => isWindows() ? 'windows' : (isMac() ? 'macos' : 'linux');
const architecture = () => process.arch === 'arm64' ? 'aarch64' : 'x86_64';
const serviceUser = () => process.env.DB_OPERATOR_SERVICE_USER || (isMac() ? '_dboperator' : 'db-operator');
const serviceLabel = () => process.env.DB_OPERATOR_SERVICE_LABEL || 'com.al.db-operator';
const privateHome = () => process.env.DB_OPERATOR_HOME
  || (isWindows() ? join(process.env.ProgramData || 'C:\\ProgramData', 'db-operator') : '/var/lib/db-operator');
const adminDefault = () => join(root, 'bin', 'service', `${platformTag()}-${architecture()}`, isWindows() ? 'db-operator-admin.exe' : 'db-operator-admin');
const defaultIssueEndpoint = () => process.env.DB_OPERATOR_ENDPOINT
  || (isWindows() ? '\\\\.\\pipe\\db-operator-v2' : (isMac() ? '/var/run/db-operator/db-operator.sock' : '/run/db-operator/db-operator.sock'));
const defaultClientConfig = () => isWindows() ? join(process.env.APPDATA || process.env.USERPROFILE || '.', 'db-operator', 'client.json') : '';
// On macOS and Linux the admin tool must run as the service identity so the registry,
// vault, and capability records keep their owner; only root may switch into it.
export function elevation(argv, executable) {
  const target = executable || process.env.DB_OPERATOR_ADMIN || adminDefault();
  if (isWindows()) return { command: target, argv, options: { windowsHide: true } };
  if (typeof process.getuid === 'function' && process.getuid() !== 0) {
    throw Error('管理界面需要管理员权限，请用 scripts/open-registry.sh 启动');
  }
  const user = serviceUser();
  if (!accountPattern.test(user)) throw Error('服务账号不合法');
  // macOS has no runuser, so the account switch goes through sudo. The server runs
  // as root, which sudo does not re-authenticate; -n is the guard that a hidden
  // window can never sit on a password prompt. /usr/bin/env carries the private
  // home, because sudo resets the environment on the way in.
  const environment = ['/usr/bin/env', `DB_OPERATOR_HOME=${privateHome()}`, target, ...argv];
  return isMac()
    ? { command: 'sudo', argv: ['-n', '-u', user, ...environment], options: {} }
    : { command: 'runuser', argv: ['-u', user, '--', ...environment], options: {} };
}
function runCapture(command, argv, options = {}) {
  return new Promise((resolveResult, reject) => {
    const child = spawn(command, argv, { ...options, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    child.stdout.on('data', data => { stdout += data; });
    child.stderr.on('data', data => { stderr = (stderr + data).slice(-4096); });
    child.on('error', error => reject(Error(`无法执行 ${command}：${error.message}`)));
    child.on('close', code => code === 0 ? resolveResult(stdout) : reject(Error(stderr.trim() || `${command} 失败（退出码 ${code}）`)));
  });
}
export function runAdmin(input, executable) {
  return new Promise((resolveResult, reject) => {
    let job;
    try { job = elevation(['manage'], executable); } catch (error) { return reject(error); }
    const child = spawn(job.command, job.argv, { ...job.options, stdio: ['pipe', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    const timeout = setTimeout(() => { child.kill(); reject(Error('管理命令超时，请刷新确认当前状态后重试')); }, 60000);
    child.stdout.on('data', data => { stdout += data; if (stdout.length > 2 * 1024 * 1024) child.kill(); });
    child.stderr.on('data', data => { stderr = (stderr + data).slice(-8192); });
    child.on('error', () => { clearTimeout(timeout); reject(Error('无法启动数据库管理程序')); });
    child.stdin.on('error', () => {}); // exit/close reports the command failure
    child.on('close', code => {
      clearTimeout(timeout);
      if (code !== 0) {
        let message = stderr || '数据库管理命令失败';
        if (input.password) message = message.replaceAll(input.password, '[redacted]');
        return reject(Error(message));
      }
      try { resolveResult(JSON.parse(stdout)); } catch { reject(Error('管理程序返回了无效结果')); }
    });
    child.stdin.end(JSON.stringify(input));
  });
}
async function body(req) {
  let text = '';
  for await (const chunk of req) {
    text += chunk;
    if (Buffer.byteLength(text) > 65536) throw Error('请求过大');
  }
  return JSON.parse(text || '{}');
}
export function runAdminCommand(argv, executable) {
  return new Promise((resolveResult, reject) => {
    let job;
    try { job = elevation(argv, executable); } catch (error) { return reject(error); }
    const child = spawn(job.command, job.argv, { ...job.options, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    const timeout = setTimeout(() => { child.kill(); reject(Error('管理命令超时，请刷新确认当前状态后重试')); }, 60000);
    child.stdout.on('data', data => { stdout += data; if (stdout.length > 2 * 1024 * 1024) child.kill(); });
    child.stderr.on('data', data => { stderr = (stderr + data).slice(-8192); });
    child.on('error', () => { clearTimeout(timeout); reject(Error('无法启动数据库管理程序')); });
    child.on('close', code => {
      clearTimeout(timeout);
      if (code !== 0) return reject(Error(stderr.trim() || '数据库管理命令失败'));
      try { resolveResult(JSON.parse(stdout)); } catch { resolveResult({ ok: true }); }
    });
  });
}
export async function agentHomeDirectory(user, run = runCapture) {
  if (!accountPattern.test(user || '')) throw Error('Agent 账号不合法');
  const output = isMac()
    ? await run('dscl', ['.', '-read', `/Users/${user}`, 'NFSHomeDirectory'])
    : await run('getent', ['passwd', user]);
  const home = isMac()
    ? ((output.match(/NFSHomeDirectory:\s*(\S+)/) || [])[1] || '')
    : (output.split(':')[5] || '');
  if (!home.startsWith('/')) throw Error(`找不到本机账号 ${user} 的家目录`);
  return home;
}
export function clientConfigFor(home) {
  return isMac()
    ? posix.join(home, 'Library', 'Application Support', 'db-operator', 'client.json')
    : posix.join(home, '.config', 'db-operator', 'client.json');
}
// Hand a service-owned staged client configuration to the Agent account: 0600 for
// the file, 0700 for its directory, so the Agent reads it without writing into the
// daemon-private directory.
export async function handOffClientConfig({ staged, output, agentUser }, run = runCapture) {
  let group;
  try { group = ['-g', (await run('id', ['-gn', agentUser])).trim()]; } catch { group = []; }
  await run('install', ['-d', '-o', agentUser, ...group, '-m', '0700', posix.dirname(output)]);
  await run('install', ['-o', agentUser, ...group, '-m', '0600', staged, output]);
}
// Issue the capability, then hand the client configuration to the Agent account;
// on macOS and Linux the daemon-private copy stays owned by the service identity.
// The issued write level must match the registered connection policy: the daemon
// compares the level carried by the capability against the one the client sends, and
// a capability issued at "none" would make an explicitly writable connection read-only.
export async function issueClientCapability({ connection, endpoint, output, agentUser, writeLevel }, command = runAdminCommand, handOff = handOffClientConfig) {
  const level = ['none', 'dml', 'ddl'].includes(writeLevel) ? writeLevel : 'none';
  if (isWindows()) {
    await command(['client', 'issue', '--connection', connection, '--endpoint', endpoint, '--output', output, '--write-level', level]);
    return output;
  }
  if (!accountPattern.test(agentUser || '')) throw Error('请填写 Agent 账号（运行 Agent 的本机用户名）');
  // posix paths throughout: these commands run on macOS and Linux, where the
  // server may not even be the host that resolves them.
  const staged = posix.join(privateHome(), 'client.json');
  try {
    await command(['client', 'issue', '--connection', connection, '--endpoint', endpoint, '--output', staged, '--write-level', level]);
    await handOff({ staged, output, agentUser });
  } finally {
    // Never leave the staging copy behind, not even when hand-off fails.
    rmSync(staged, { force: true });
  }
  return output;
}
// The registered connection owns the write policy; issuance only mirrors it.
async function registeredWriteLevel(manage, name) {
  try {
    const shown = await manage({ operation: 'show', name });
    const level = shown && shown.profile && shown.profile.write_level;
    return ['none', 'dml', 'ddl'].includes(level) ? level : 'none';
  } catch {
    return 'none';
  }
}
// Return only admin-safe fields; the capability digest/token must never leave the daemon-private record.
export function readCapability(home = process.env.DB_OPERATOR_HOME) {
  if (!home) return { active: false, connection: null, client_path: null };
  try {
    const record = JSON.parse(readFileSync(join(home, 'capability.json'), 'utf8'));
    return { active: true, connection: typeof record.connection === 'string' ? record.connection : null, client_path: typeof record.client_path === 'string' ? record.client_path : null };
  } catch {
    return { active: false, connection: null, client_path: null };
  }
}
export function restartService(name = process.env.DB_OPERATOR_SERVICE_NAME || 'db-operator') {
  return new Promise((resolveResult, reject) => {
    if (!/^[A-Za-z0-9_.-]{1,64}$/.test(name || '')) return reject(Error('服务名不合法'));
    const [command, argv] = isWindows()
      ? ['powershell.exe', ['-NoProfile', '-Command', `Restart-Service -Name '${name}'`]]
      : isMac()
        ? ['launchctl', ['kickstart', '-k', `system/${serviceLabel()}`]]
        : ['systemctl', ['restart', name]];
    const child = spawn(command, argv, { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
    let stderr = '';
    const timeout = setTimeout(() => { child.kill(); reject(Error('重启服务超时')); }, 120000);
    child.stderr.on('data', data => { stderr = (stderr + data).slice(-4096); });
    child.on('error', () => { clearTimeout(timeout); reject(Error('无法调用服务管理命令')); });
    child.on('close', code => {
      clearTimeout(timeout);
      if (code !== 0) return reject(Error(stderr.trim() || '重启服务失败'));
      resolveResult(true);
    });
  });
}
export function createRegistryServer({ manage = runAdmin, command = runAdminCommand, capability = readCapability, restart = restartService, issue = issueClientCapability } = {}) {
  const token = randomBytes(32).toString('hex');
  const confirmations = new Map();
  let busy = false;
  return http.createServer(async (req, res) => {
    const json = (code, value) => { res.writeHead(code, { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' }); res.end(JSON.stringify(value)); };
    const origin = 'http://127.0.0.1:' + req.socket.localPort;
    if (req.headers.host !== new URL(origin).host || (req.headers.origin && req.headers.origin !== origin)) return json(403, { error: '只允许本机管理页面访问' });
    try {
      if (req.method === 'GET' && req.url === '/') {
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store', 'x-frame-options': 'DENY', 'referrer-policy':'no-referrer' });
        return res.end(readFileSync(join(root, 'ui/index.html')));
      }
      if (req.method === 'GET' && req.url === '/api/session') return json(200, { token, service: 'db-operator', version: 2, platform: platformTag(), requires_agent_user: !isWindows() });
      if (req.headers['x-db-operator-ui-token'] !== token) return json(403, { error: '管理会话无效，请刷新页面' });
      if (req.method === 'GET' && req.url === '/api/connections') return json(200, await manage({ operation: 'list' }));
      if (req.method === 'GET' && req.url === '/api/capability') return json(200, { ...capability(), defaults: { endpoint: defaultIssueEndpoint(), output: defaultClientConfig(), agent_user: '' } });
      if (req.method === 'GET' && req.url.startsWith('/api/defaults')) {
        if (isWindows()) return json(200, { endpoint: defaultIssueEndpoint(), output: defaultClientConfig() });
        const agent = new URL(req.url, 'http://127.0.0.1').searchParams.get('agent') || '';
        try {
          return json(200, { endpoint: defaultIssueEndpoint(), output: clientConfigFor(await agentHomeDirectory(agent)) });
        } catch (error) { return json(400, { error: error.message }); }
      }
      const route = /^\/api\/connections\/([^/]+)(\/delete-confirmation)?$/.exec(req.url);
      const name = route && decodeURIComponent(route[1]);
      if (route && !aliasPattern.test(name)) return json(400, { error: '连接别名不合法' });
      if (req.method === 'GET' && route && !route[2]) return json(200, await manage({ operation: 'show', name }));
      if (busy) return json(409, { error: '另一项管理操作尚未完成，请稍后重试' });
      busy = true;
      try {
        if (req.method === 'POST' && route?.[2]) {
          const snapshot = await manage({ operation: 'show', name });
          for (const [key, value] of confirmations) if (value.expires < Date.now()) confirmations.delete(key);
          if (confirmations.size >= 100) confirmations.clear();
          const confirmation = randomBytes(24).toString('hex');
          confirmations.set(confirmation, { name, expires: Date.now() + 120000, fingerprint: fingerprint(snapshot) });
          return json(200, { confirmation, name });
        }
        if (req.method === 'DELETE' && route && !route[2]) {
          const input = await body(req), pending = confirmations.get(input.confirmation);
          if (!pending || pending.name !== name || pending.expires < Date.now() || input.confirmed_name !== name) return json(409, { error: '请再次确认并输入完整连接别名' });
          const current = await manage({ operation: 'show', name });
          confirmations.delete(input.confirmation);
          if (fingerprint(current) !== pending.fingerprint) return json(409, { error: '连接已发生变化，请重新确认删除' });
          return json(200, await manage({ operation: 'delete', name, confirmed_name: name }));
        }
        if (req.method === 'POST' && req.url === '/api/schema-refresh') {
          return json(200, await command(['schema', 'refresh']));
        }
        if (req.method === 'POST' && req.url === '/api/capability/issue') {
          const input = await body(req);
          const connection = String(input.connection || '');
          const endpoint = String(input.endpoint || '').trim();
          const output = String(input.output || '').trim();
          if (!aliasPattern.test(connection)) return json(400, { error: '连接别名不合法' });
          if (!endpoint || endpoint.length > 260 || !output || output.length > 260 || /[\0-\x1f]/.test(endpoint + output)) return json(400, { error: '端点或客户端配置路径不合法' });
          const before = capability();
          const writeLevel = await registeredWriteLevel(manage, connection);
          const written = await issue({ connection, endpoint, output, agentUser: String(input.agent_user || '').trim(), writeLevel });
          let service_restarted = false;
          if (input.restart !== false) { await restart(); service_restarted = true; }
          return json(200, { issued: true, connection, client_config: written, write_level: writeLevel, replaced: before.active ? before.connection : null, service_restarted });
        }
        if ((req.method === 'POST' && req.url === '/api/connections') || (req.method === 'PUT' && route && !route[2])) {
          const input = await body(req);
          const selectedName = name || input.name;
          if (!aliasPattern.test(selectedName || '')) return json(400, { error: '连接别名不合法' });
          const result = await manage({ operation: name ? 'update' : 'create', name: selectedName, profile: input.profile, password: input.password || '', test: input.test === true });
          confirmations.clear();
          // Creating a connection may auto-issue the first capability so the Agent
          // client.json exists right away; updates never replace an authorization.
          if (req.method === 'POST' && input.issue === true) {
            const current = capability();
            if (current.active && current.connection && current.connection !== selectedName) {
              result.issue_skipped = 'capability-exists';
              result.existing_connection = current.connection;
            } else {
              const endpoint = String(input.endpoint || '').trim() || defaultIssueEndpoint();
              const output = String(input.output || '').trim() || defaultClientConfig();
              if (!output || endpoint.length > 260 || output.length > 260 || /[\0-\x1f]/.test(endpoint + output)) {
                result.issue_error = isWindows()
                  ? '端点或客户端配置路径不合法，连接已保存，请手动签发授权'
                  : '请先在"Agent 查询授权"卡片填写 Agent 账号以确定 client.json 路径；连接已保存，可稍后签发授权';
              } else {
                try {
                  const writeLevel = await registeredWriteLevel(manage, selectedName);
                  result.client_config = await issue({ connection: selectedName, endpoint, output, agentUser: String(input.agent_user || '').trim(), writeLevel });
                  result.issued = true;
                  result.write_level = writeLevel;
                  if (input.restart !== false) { await restart(); result.service_restarted = true; }
                } catch (error) {
                  result.issue_error = error.message;
                }
              }
            }
          }
          return json(200, result);
        }
        return json(404, { error: '接口不存在' });
      } finally { busy = false; }
    } catch (error) { json(400, { error: error.message }); }
  });
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const port = Number(process.env.DB_OPERATOR_UI_PORT || 17842);
  const server = createRegistryServer();
  server.on('error', error => {
    if (error.code === 'EADDRINUSE') {
      console.error(`db-operator UI: 端口 ${port} 已被占用。可能已有一个管理界面在运行（请直接访问 http://127.0.0.1:${port}/），或其它程序占用了该端口；也可设置 DB_OPERATOR_UI_PORT 换端口后重试。`);
    } else {
      console.error('db-operator UI: 无法监听管理端口：' + (error.message || error.code));
    }
    process.exit(1);
  });
  server.listen(port, '127.0.0.1', () => console.log('db-operator UI: http://127.0.0.1:' + port + '/'));
}
