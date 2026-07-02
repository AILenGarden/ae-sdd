'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { Bot, Cable, Check, Copy, KeyRound, RefreshCw, TerminalSquare } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@nubase/ui';
import { apiFetch, API_BASE, type ApiError } from '@/lib/api';
import { useSession } from '@/lib/session';
import { useProjectRef } from '@/lib/route-params';

type ClientId = 'codex' | 'claude-code' | 'cursor' | 'idea' | 'generic';

interface ConnectConfig {
  client: ClientId;
  mcp: {
    endpoint: string;
    headers: {
      apikey: string;
    };
  };
  env: Record<string, string>;
  aiGateway: {
    openAI: {
      baseUrl: string;
      apiKey: string;
    };
    anthropic: {
      baseUrl: string;
      authToken: string;
    };
  };
  templates: {
    mcpServers: {
      nubase: {
        url: string;
        headers: {
          apikey: string;
        };
      };
    };
  };
}

const CLIENTS: { id: ClientId; label: string }[] = [
  { id: 'codex', label: 'Codex' },
  { id: 'claude-code', label: 'Claude Code' },
  { id: 'cursor', label: 'Cursor' },
  { id: 'idea', label: 'IDEA' },
  { id: 'generic', label: 'Generic' },
];

export default function ConnectAgentPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const apikey = project?.apikey ?? '';
  const [client, setClient] = useState<ClientId>('codex');
  const [config, setConfig] = useState<ConnectConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch<ConnectConfig>(
        `/agent/v1/connect-config?client=${encodeURIComponent(client)}`,
        { apikey: apikey || undefined }
      );
      setConfig(res);
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load agent configuration.');
    } finally {
      setLoading(false);
    }
  }, [apikey, client]);

  useEffect(() => {
    load();
  }, [load]);

  const resolved = useMemo(() => resolveConfig(config, apikey), [config, apikey]);
  const mcpJson = useMemo(() => JSON.stringify(resolved.templates, null, 2), [resolved.templates]);
  const stdioJson = useMemo(
    () => JSON.stringify(stdioTemplate(client, resolved.env.NUBASE_API_KEY ?? 'YOUR_NUBASE_PROJECT_KEY'), null, 2),
    [client, resolved.env.NUBASE_API_KEY]
  );
  const envText = useMemo(
    () => Object.entries(resolved.env).map(([k, v]) => `${k}=${v}`).join('\n'),
    [resolved.env]
  );

  async function copy(label: string, value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      setTimeout(() => setCopied(null), 1500);
    } catch {
      // Clipboard availability depends on browser permissions.
    }
  }

  return (
    <div className="flex h-full flex-col bg-background">
      <header className="border-b border-border px-5 py-4">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-md border border-border bg-card">
              <Cable className="h-4 w-4 text-brand" />
            </div>
            <div>
              <h1 className="text-base font-semibold">Connect Agent</h1>
              <p className="text-xs text-muted-foreground">
                MCP tools and model gateway settings for <span className="font-mono">{projectRef}</span>.
              </p>
            </div>
          </div>
          <Button size="sm" variant="outline" onClick={load} disabled={loading}>
            <RefreshCw className={'h-3.5 w-3.5 ' + (loading ? 'animate-spin' : '')} />
            Refresh
          </Button>
        </div>
        <div className="mt-4 flex flex-wrap gap-1">
          {CLIENTS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setClient(item.id)}
              className={
                'inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors ' +
                (client === item.id
                  ? 'bg-accent text-accent-foreground'
                  : 'text-muted-foreground hover:bg-accent/60 hover:text-foreground')
              }
            >
              <Bot className="h-3.5 w-3.5" />
              {item.label}
            </button>
          ))}
        </div>
      </header>

      <main className="flex-1 overflow-auto p-5">
        {error ? (
          <Card className="mb-4">
            <CardContent className="p-4 text-sm text-destructive">{error}</CardContent>
          </Card>
        ) : null}

        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.15fr)_minmax(360px,0.85fr)]">
          <section className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-base">
                  <TerminalSquare className="h-4 w-4" />
                  Tools / MCP
                </CardTitle>
                <CardDescription>Use this when an agent needs Nubase tools for memory, schema, SQL, storage, and auth.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4 text-sm">
                <Row label="Endpoint">
                  <CopyLine value={resolved.mcp.endpoint} copied={copied === 'mcp-url'} onCopy={() => copy('mcp-url', resolved.mcp.endpoint)} />
                </Row>
                <Row label="Header">
                  <CopyLine value={`apikey: ${resolved.mcp.headers.apikey}`} copied={copied === 'mcp-key'} onCopy={() => copy('mcp-key', resolved.mcp.headers.apikey)} />
                </Row>
                <CodeBlock label="stdio MCP JSON" value={stdioJson} copied={copied === 'stdio-json'} onCopy={() => copy('stdio-json', stdioJson)} />
                <CodeBlock label="remote MCP JSON" value={mcpJson} copied={copied === 'mcp-json'} onCopy={() => copy('mcp-json', mcpJson)} />
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2 text-base">
                  <KeyRound className="h-4 w-4" />
                  Model Gateway
                </CardTitle>
                <CardDescription>Use this when the agent should route model calls through Nubase AI Gateway.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4 text-sm">
                <Row label="OpenAI">
                  <CopyLine value={resolved.aiGateway.openAI.baseUrl} copied={copied === 'openai-url'} onCopy={() => copy('openai-url', resolved.aiGateway.openAI.baseUrl)} />
                </Row>
                <Row label="Anthropic">
                  <CopyLine value={resolved.aiGateway.anthropic.baseUrl} copied={copied === 'anthropic-url'} onCopy={() => copy('anthropic-url', resolved.aiGateway.anthropic.baseUrl)} />
                </Row>
                <CodeBlock label=".env" value={envText} copied={copied === 'env'} onCopy={() => copy('env', envText)} />
              </CardContent>
            </Card>
          </section>

          <aside className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="text-base">Connection Contract</CardTitle>
                <CardDescription>Keep these boundaries clear when configuring agents.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 text-xs text-muted-foreground">
                <Boundary title="MCP tools" text="Expose project operations: memoryContext, memoryWrite, table inspection, SQL execution, and future Auth/Storage tools." />
                <Boundary title="AI Gateway" text="Routes model calls through Nubase for provider abstraction, usage logs, pricing, and gateway keys." />
                <Boundary title="Key handling" text="Use service-role keys only in trusted agent environments. Do not place them in generated browser code." />
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-base">Resolved Values</CardTitle>
                <CardDescription>Rendered from the current Studio session.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 text-xs">
                <MiniRow label="Client" value={clientLabel(client)} />
                <MiniRow label="Project" value={projectRef} />
                <MiniRow label="API base" value={API_BASE} />
                <div className="flex items-center justify-between gap-3">
                  <span className="text-muted-foreground">Project key</span>
                  {apikey ? <Badge variant="success">loaded</Badge> : <Badge variant="outline">placeholder</Badge>}
                </div>
              </CardContent>
            </Card>
          </aside>
        </div>
      </main>
    </div>
  );
}

function resolveConfig(config: ConnectConfig | null, projectKey: string): ConnectConfig {
  const baseUrl = API_BASE;
  const key = projectKey || 'YOUR_NUBASE_PROJECT_KEY';
  const gatewayKey = 'YOUR_NUBASE_AI_GATEWAY_KEY';
  const fallback: ConnectConfig = {
    client: 'generic',
    mcp: { endpoint: `${baseUrl}/mcp`, headers: { apikey: key } },
    env: {
      NUBASE_URL: baseUrl,
      NUBASE_API_KEY: key,
      OPENAI_BASE_URL: `${baseUrl}/v1`,
      OPENAI_API_KEY: gatewayKey,
      ANTHROPIC_BASE_URL: baseUrl,
      ANTHROPIC_AUTH_TOKEN: gatewayKey,
    },
    aiGateway: {
      openAI: { baseUrl: `${baseUrl}/v1`, apiKey: gatewayKey },
      anthropic: { baseUrl, authToken: gatewayKey },
    },
    templates: {
      mcpServers: {
        nubase: {
          url: `${baseUrl}/mcp`,
          headers: { apikey: key },
        },
      },
    },
  };
  const next = config ?? fallback;
  return {
    ...next,
    mcp: { ...next.mcp, endpoint: `${baseUrl}/mcp`, headers: { apikey: key } },
    env: {
      ...next.env,
      NUBASE_URL: baseUrl,
      NUBASE_API_KEY: key,
      OPENAI_BASE_URL: `${baseUrl}/v1`,
      ANTHROPIC_BASE_URL: baseUrl,
    },
    aiGateway: {
      openAI: { ...next.aiGateway.openAI, baseUrl: `${baseUrl}/v1` },
      anthropic: { ...next.aiGateway.anthropic, baseUrl },
    },
    templates: {
      mcpServers: {
        nubase: {
          url: `${baseUrl}/mcp`,
          headers: { apikey: key },
        },
      },
    },
  };
}

function stdioTemplate(client: ClientId, projectKey: string) {
  return {
    mcpServers: {
      nubase: {
        command: 'npx',
        args: ['nubase_cli'],
        env: {
          NUBASE_URL: API_BASE,
          NUBASE_PROJECT_KEY: projectKey,
          NUBASE_AGENT_ID: client,
        },
      },
    },
  };
}

function clientLabel(client: ClientId) {
  return CLIENTS.find((c) => c.id === client)?.label ?? 'Generic';
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid gap-2 sm:grid-cols-[120px_1fr] sm:items-start">
      <span className="pt-1 text-xs text-muted-foreground">{label}</span>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

function CopyLine({ value, copied, onCopy }: { value: string; copied: boolean; onCopy: () => void }) {
  return (
    <div className="flex min-w-0 items-center gap-2">
      <code className="min-w-0 flex-1 break-all rounded-md border border-border bg-muted/30 px-2 py-1.5 font-mono text-xs">
        {value}
      </code>
      <Button size="icon" variant="ghost" onClick={onCopy} aria-label="Copy value">
        {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
      </Button>
    </div>
  );
}

function CodeBlock({ label, value, copied, onCopy }: { label: string; value: string; copied: boolean; onCopy: () => void }) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs text-muted-foreground">{label}</span>
        <Button size="sm" variant="outline" onClick={onCopy}>
          {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
          {copied ? 'Copied' : 'Copy'}
        </Button>
      </div>
      <pre className="max-h-[320px] overflow-auto rounded-md border border-border bg-muted/30 p-3 text-xs leading-relaxed">
        <code>{value}</code>
      </pre>
    </div>
  );
}

function Boundary({ title, text }: { title: string; text: string }) {
  return (
    <div className="rounded-md border border-border p-3">
      <div className="mb-1 text-xs font-medium text-foreground">{title}</div>
      <p>{text}</p>
    </div>
  );
}

function MiniRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      <code className="min-w-0 truncate font-mono">{value}</code>
    </div>
  );
}
