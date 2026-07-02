'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Activity,
  Bot,
  Copy,
  KeyRound,
  Plus,
  RefreshCw,
  Route,
  Save,
  SlidersHorizontal,
  Trash2,
  WalletCards,
  XCircle,
} from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardContent,
  Dialog,
  DialogBody,
  DialogFooter,
  DialogHeader,
  Input,
  Label,
} from '@nubase/ui';
import type { LucideIcon } from 'lucide-react';
import { apiFetch, API_BASE, type ApiError } from '@/lib/api';
import { isProjectReady, useSession } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { useProjectRef } from '@/lib/route-params';

type Tab = 'overview' | 'routes' | 'keys' | 'logs' | 'pricing';

interface UsageOverview {
  totalTokens: number;
  totalRequests: number;
  totalCostUsd: number | string;
  avgFirstTokenLatencyMs: number;
  series: { date: string; tokens: number; requests: number }[];
}

interface Upstream {
  id: number;
  name: string;
  baseUrl: string;
  authTokenSet: boolean;
  provider: 'CLAUDE' | 'OPENAI';
  channelCode?: string | null;
  supportedModels?: string[];
  chatCompletionsPath?: string;
  description?: string | null;
  isDefault: boolean;
  isActive: boolean;
  timeoutMs: number;
  maxRetries: number;
  priority: number;
  maxInputTokens?: number | null;
  lastUsedAt?: string | null;
  healthStatus?: string | null;
}

interface GatewayKey {
  id: number;
  name?: string | null;
  prefix?: string | null;
  isActive: boolean;
  createdAt?: string | null;
  lastUsedAt?: string | null;
  expiresAt?: string | null;
  revokedAt?: string | null;
  apiKey?: string;
}

interface UsageLog {
  id: number;
  apiKeyId?: number | null;
  userId?: string | null;
  keyPrefix?: string | null;
  authMode?: string | null;
  requestId?: string | null;
  model?: string | null;
  endpoint?: string | null;
  statusCode?: number | null;
  inputTokens?: number | null;
  outputTokens?: number | null;
  totalTokens?: number | null;
  durationMs?: number | null;
  firstTokenLatencyMs?: number | null;
  costUsd?: number | string | null;
  errorMessage?: string | null;
  createdAt?: string | null;
}

interface LogsPage {
  content: UsageLog[];
  totalElements: number;
}

interface Pricing {
  id: number;
  model: string;
  provider: string;
  displayName?: string | null;
  inputPricePer1MUsd: number | string;
  outputPricePer1MUsd: number | string;
  cacheCreationPricePer1MUsd: number | string;
  cacheReadPricePer1MUsd: number | string;
  currency: string;
  isActive: boolean;
  sortOrder: number;
}

const EMPTY_UPSTREAM = {
  name: '',
  provider: 'OPENAI',
  channelCode: 'openai',
  baseUrl: '',
  authToken: '',
  priority: 10,
  isDefault: false,
  isActive: true,
  timeoutMs: 60000,
  maxRetries: 2,
  maxInputTokens: '',
  chatCompletionsPath: '/v1/chat/completions',
  supportedModels: '',
  description: '',
};

const TABS: { value: Tab; icon: LucideIcon; label: string }[] = [
  { value: 'overview', icon: Activity, label: 'Overview' },
  { value: 'routes', icon: Route, label: 'Routes' },
  { value: 'keys', icon: KeyRound, label: 'API Keys' },
  { value: 'logs', icon: SlidersHorizontal, label: 'Logs' },
  { value: 'pricing', icon: WalletCards, label: 'Pricing' },
];

export default function AiGatewayPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <AiGatewayInner projectRef={projectRef} />;
}

function AiGatewayInner({ projectRef }: { projectRef: string }) {
  const { project } = useSession();
  const apikey = project!.apikey;
  const [tab, setTab] = useState<Tab>('overview');
  const [overview, setOverview] = useState<UsageOverview | null>(null);
  const [upstreams, setUpstreams] = useState<Upstream[]>([]);
  const [keys, setKeys] = useState<GatewayKey[]>([]);
  const [logs, setLogs] = useState<UsageLog[]>([]);
  const [pricing, setPricing] = useState<Pricing[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [o, u, k, l, p] = await Promise.all([
        apiFetch<UsageOverview>('/ai-gateway/admin/v1/usage/overview', { apikey }),
        apiFetch<Upstream[]>('/ai-gateway/admin/v1/upstreams', { apikey }),
        apiFetch<{ data: GatewayKey[] }>('/ai-gateway/admin/v1/keys', { apikey }),
        apiFetch<LogsPage>('/ai-gateway/admin/v1/usage/logs?size=50', { apikey }),
        apiFetch<Pricing[]>('/ai-gateway/admin/v1/pricing/all', { apikey }),
      ]);
      setOverview(o);
      setUpstreams(u);
      setKeys(k.data);
      setLogs(l.content);
      setPricing(p);
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load AI Gateway.');
    } finally {
      setLoading(false);
    }
  }, [apikey]);

  useEffect(() => { load(); }, [load]);

  const maxSeriesTokens = useMemo(
    () => Math.max(1, ...(overview?.series ?? []).map((s) => s.tokens)),
    [overview]
  );

  return (
    <div className="flex h-full flex-col bg-background">
      <header className="border-b border-border px-5 py-4">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-md border border-border bg-card">
              <Bot className="h-4 w-4 text-brand" />
            </div>
            <div>
              <h1 className="text-base font-semibold">AI Gateway</h1>
              <p className="text-xs text-muted-foreground">
                Project routing, keys, pricing and usage for <span className="font-mono">{projectRef}</span>.
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <code className="hidden rounded-md border border-border bg-card px-2 py-1 text-xs text-muted-foreground md:block">
              {API_BASE}/v1
            </code>
            <Button size="sm" variant="outline" onClick={load} disabled={loading}>
              <RefreshCw className={'h-3.5 w-3.5 ' + (loading ? 'animate-spin' : '')} />
              Refresh
            </Button>
          </div>
        </div>
        <div className="mt-4 flex flex-wrap gap-1">
          {TABS.map(({ value, icon: Icon, label }) => (
            <button
              key={value as string}
              type="button"
              onClick={() => setTab(value)}
              className={
                'inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors ' +
                (tab === value
                  ? 'bg-accent text-accent-foreground'
                  : 'text-muted-foreground hover:bg-accent/60 hover:text-foreground')
              }
            >
              <Icon className="h-3.5 w-3.5" />
              {label}
            </button>
          ))}
        </div>
      </header>

      <main className="flex-1 overflow-auto p-5">
        {error ? (
          <Card>
            <CardContent className="flex items-center gap-2 p-4 text-sm text-destructive">
              <XCircle className="h-4 w-4" /> {error}
            </CardContent>
          </Card>
        ) : null}

        {tab === 'overview' ? (
          <OverviewPanel overview={overview} maxSeriesTokens={maxSeriesTokens} loading={loading} />
        ) : null}
        {tab === 'routes' ? (
          <RoutesPanel apikey={apikey} rows={upstreams} onChanged={load} />
        ) : null}
        {tab === 'keys' ? (
          <KeysPanel apikey={apikey} rows={keys} onChanged={load} />
        ) : null}
        {tab === 'logs' ? (
          <LogsPanel rows={logs} />
        ) : null}
        {tab === 'pricing' ? (
          <PricingPanel apikey={apikey} rows={pricing} onChanged={load} />
        ) : null}
      </main>
    </div>
  );
}

function OverviewPanel({
  overview,
  maxSeriesTokens,
  loading,
}: {
  overview: UsageOverview | null;
  maxSeriesTokens: number;
  loading: boolean;
}) {
  const cards = [
    ['Requests', overview?.totalRequests ?? 0],
    ['Tokens', overview?.totalTokens ?? 0],
    ['Cost USD', `$${Number(overview?.totalCostUsd ?? 0).toFixed(4)}`],
    ['Avg first token', `${overview?.avgFirstTokenLatencyMs ?? 0} ms`],
  ];
  return (
    <div className="space-y-5">
      <div className="grid gap-3 md:grid-cols-4">
        {cards.map(([label, value]) => (
          <Card key={label}>
            <CardContent className="p-4">
              <p className="text-xs text-muted-foreground">{label}</p>
              <p className="mt-2 text-xl font-semibold">{value}</p>
            </CardContent>
          </Card>
        ))}
      </div>
      <Card>
        <CardContent className="p-4">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold">14 day trend</h2>
              <p className="text-xs text-muted-foreground">Token volume by day.</p>
            </div>
            {loading ? <Badge variant="outline">Loading</Badge> : null}
          </div>
          <div className="flex h-56 items-end gap-2">
            {(overview?.series ?? []).map((point) => (
              <div key={point.date} className="flex min-w-0 flex-1 flex-col items-center gap-2">
                <div className="flex h-44 w-full items-end rounded-sm bg-muted/30">
                  <div
                    className="w-full rounded-sm bg-brand/80"
                    style={{ height: `${Math.max(4, (point.tokens / maxSeriesTokens) * 100)}%` }}
                    title={`${point.tokens} tokens`}
                  />
                </div>
                <span className="text-[10px] text-muted-foreground">{point.date}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function RoutesPanel({ apikey, rows, onChanged }: { apikey: string; rows: Upstream[]; onChanged: () => void }) {
  const [editing, setEditing] = useState<Upstream | null>(null);
  const [creating, setCreating] = useState(false);
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold">Routes</h2>
          <p className="text-xs text-muted-foreground">Priority ordered upstreams with failover.</p>
        </div>
        <Button size="sm" variant="brand" onClick={() => setCreating(true)}>
          <Plus className="h-3.5 w-3.5" /> New route
        </Button>
      </div>
      <Table
        headers={['Name', 'Provider', 'Channel', 'Models', 'Priority', 'Status', '']}
        rows={rows.map((r) => [
          <div key="name">
            <p className="font-medium">{r.name}</p>
            <p className="max-w-md truncate text-xs text-muted-foreground">{r.baseUrl}</p>
          </div>,
          r.provider,
          r.channelCode ?? '-',
          <span key="models" className="text-xs text-muted-foreground">
            {(r.supportedModels ?? []).slice(0, 2).join(', ') || 'any'}
          </span>,
          String(r.priority),
          <StatusBadge key="status" active={r.isActive} label={r.isDefault ? 'default' : r.healthStatus ?? 'active'} />,
          <Button key="edit" size="sm" variant="outline" onClick={() => setEditing(r)}>Edit</Button>,
        ])}
      />
      <RouteDialog
        open={creating || Boolean(editing)}
        route={editing}
        apikey={apikey}
        onClose={() => { setCreating(false); setEditing(null); }}
        onSaved={onChanged}
      />
    </div>
  );
}

function RouteDialog({
  open,
  route,
  apikey,
  onClose,
  onSaved,
}: {
  open: boolean;
  route: Upstream | null;
  apikey: string;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [draft, setDraft] = useState(EMPTY_UPSTREAM);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    if (!route) {
      setDraft(EMPTY_UPSTREAM);
    } else {
      setDraft({
        name: route.name,
        provider: route.provider,
        channelCode: route.channelCode ?? route.provider.toLowerCase(),
        baseUrl: route.baseUrl,
        authToken: '',
        priority: route.priority,
        isDefault: route.isDefault,
        isActive: route.isActive,
        timeoutMs: route.timeoutMs,
        maxRetries: route.maxRetries,
        maxInputTokens: route.maxInputTokens?.toString() ?? '',
        chatCompletionsPath: route.chatCompletionsPath ?? '/v1/chat/completions',
        supportedModels: (route.supportedModels ?? []).join(', '),
        description: route.description ?? '',
      });
    }
    setError(null);
  }, [open, route]);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const body = {
        ...draft,
        supportedModels: draft.supportedModels.split(',').map((s) => s.trim()).filter(Boolean),
        maxInputTokens: draft.maxInputTokens ? Number(draft.maxInputTokens) : null,
      };
      await apiFetch(route ? `/ai-gateway/admin/v1/upstreams/${route.id}` : '/ai-gateway/admin/v1/upstreams', {
        apikey,
        method: route ? 'PUT' : 'POST',
        body,
      });
      await apiFetch('/ai-gateway/admin/v1/upstreams/cache/refresh', { apikey, method: 'POST' });
      onSaved();
      onClose();
    } catch (err) {
      setError((err as ApiError).message ?? 'Save failed.');
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onClose={onClose} size="max-w-2xl">
      <DialogHeader title={route ? 'Edit route' : 'New route'} description="Auth token is write-only and never returned by the API." onClose={onClose} />
      <DialogBody className="grid gap-3 md:grid-cols-2">
        <Field label="Name" value={draft.name} onChange={(v) => setDraft({ ...draft, name: v })} />
        <Field label="Provider" value={draft.provider} onChange={(v) => setDraft({ ...draft, provider: v })} />
        <Field label="Channel" value={draft.channelCode} onChange={(v) => setDraft({ ...draft, channelCode: v })} />
        <Field label="Priority" type="number" value={String(draft.priority)} onChange={(v) => setDraft({ ...draft, priority: Number(v) })} />
        <Field className="md:col-span-2" label="Base URL" value={draft.baseUrl} onChange={(v) => setDraft({ ...draft, baseUrl: v })} />
        <Field className="md:col-span-2" label={route?.authTokenSet ? 'Auth token (leave blank to keep)' : 'Auth token'} type="password" value={draft.authToken} onChange={(v) => setDraft({ ...draft, authToken: v })} />
        <Field label="Timeout ms" type="number" value={String(draft.timeoutMs)} onChange={(v) => setDraft({ ...draft, timeoutMs: Number(v) })} />
        <Field label="Max input tokens" value={draft.maxInputTokens} onChange={(v) => setDraft({ ...draft, maxInputTokens: v })} />
        <Field className="md:col-span-2" label="Supported models" value={draft.supportedModels} onChange={(v) => setDraft({ ...draft, supportedModels: v })} />
        <label className="flex items-center gap-2 text-xs">
          <input type="checkbox" checked={draft.isDefault} onChange={(e) => setDraft({ ...draft, isDefault: e.target.checked })} />
          Default route
        </label>
        <label className="flex items-center gap-2 text-xs">
          <input type="checkbox" checked={draft.isActive} onChange={(e) => setDraft({ ...draft, isActive: e.target.checked })} />
          Active
        </label>
        {error ? <p className="md:col-span-2 text-xs text-destructive">{error}</p> : null}
      </DialogBody>
      <DialogFooter>
        <Button variant="outline" onClick={onClose}>Cancel</Button>
        <Button variant="brand" onClick={save} disabled={saving}>
          <Save className="h-3.5 w-3.5" /> Save
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function KeysPanel({ apikey, rows, onChanged }: { apikey: string; rows: GatewayKey[]; onChanged: () => void }) {
  const [newKey, setNewKey] = useState<string | null>(null);
  const issue = async () => {
    const name = window.prompt('Key name');
    if (!name) return;
    const res = await apiFetch<GatewayKey>('/ai-gateway/admin/v1/keys', {
      apikey,
      method: 'POST',
      body: { name },
    });
    setNewKey(res.apiKey ?? null);
    onChanged();
  };
  const revoke = async (id: number) => {
    if (!window.confirm('Revoke this key?')) return;
    await apiFetch(`/ai-gateway/admin/v1/keys/${id}`, { apikey, method: 'DELETE' });
    onChanged();
  };
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold">API Keys</h2>
          <p className="text-xs text-muted-foreground">Self-routing keys for CLI and server-side AI calls.</p>
        </div>
        <Button size="sm" variant="brand" onClick={issue}>
          <Plus className="h-3.5 w-3.5" /> Issue key
        </Button>
      </div>
      {newKey ? (
        <Card>
          <CardContent className="flex items-center justify-between gap-3 p-4">
            <div>
              <p className="text-xs font-medium">Copy this key now. It will not be shown again.</p>
              <code className="mt-2 block break-all rounded-md bg-muted p-2 text-xs">{newKey}</code>
            </div>
            <Button size="icon" variant="outline" onClick={() => navigator.clipboard.writeText(newKey)}>
              <Copy className="h-4 w-4" />
            </Button>
          </CardContent>
        </Card>
      ) : null}
      <Table
        headers={['Name', 'Prefix', 'Created', 'Last used', 'Status', '']}
        rows={rows.map((k) => [
          k.name ?? 'Untitled',
          <code key="prefix" className="text-xs">{k.prefix ?? '-'}</code>,
          formatDate(k.createdAt),
          formatDate(k.lastUsedAt),
          <StatusBadge key="status" active={k.isActive && !k.revokedAt} label={k.revokedAt ? 'revoked' : 'active'} />,
          <Button key="revoke" size="sm" variant="outline" onClick={() => revoke(k.id)}>
            <Trash2 className="h-3.5 w-3.5" /> Revoke
          </Button>,
        ])}
      />
    </div>
  );
}

function LogsPanel({ rows }: { rows: UsageLog[] }) {
  return (
    <div className="space-y-3">
      <div>
        <h2 className="text-sm font-semibold">Request logs</h2>
        <p className="text-xs text-muted-foreground">Recent gateway requests, including API-key and user-JWT calls.</p>
      </div>
      <Table
        headers={['When', 'Model', 'Endpoint', 'Auth', 'Status', 'Tokens', 'Cost', 'Latency']}
        rows={rows.map((l) => [
          formatDate(l.createdAt),
          l.model ?? '-',
          <span key="endpoint" className="max-w-xs truncate">{l.endpoint ?? '-'}</span>,
          l.authMode ?? (l.userId ? 'USER_JWT' : 'GATEWAY_KEY'),
          <StatusBadge key="status" active={(l.statusCode ?? 500) < 400} label={String(l.statusCode ?? '-')} />,
          String(l.totalTokens ?? 0),
          `$${Number(l.costUsd ?? 0).toFixed(5)}`,
          `${l.firstTokenLatencyMs ?? l.durationMs ?? 0} ms`,
        ])}
      />
    </div>
  );
}

function PricingPanel({ apikey, rows, onChanged }: { apikey: string; rows: Pricing[]; onChanged: () => void }) {
  const toggle = async (p: Pricing) => {
    await apiFetch(`/ai-gateway/admin/v1/pricing/${p.id}`, {
      apikey,
      method: 'PUT',
      body: { ...p, isActive: !p.isActive },
    });
    onChanged();
  };
  return (
    <div className="space-y-3">
      <div>
        <h2 className="text-sm font-semibold">Model pricing</h2>
        <p className="text-xs text-muted-foreground">Used for cost analytics only.</p>
      </div>
      <Table
        headers={['Model', 'Provider', 'Input / 1M', 'Output / 1M', 'Currency', 'Status', '']}
        rows={rows.map((p) => [
          p.displayName || p.model,
          p.provider,
          `$${Number(p.inputPricePer1MUsd ?? 0).toFixed(4)}`,
          `$${Number(p.outputPricePer1MUsd ?? 0).toFixed(4)}`,
          p.currency,
          <StatusBadge key="status" active={p.isActive} label={p.isActive ? 'active' : 'inactive'} />,
          <Button key="toggle" size="sm" variant="outline" onClick={() => toggle(p)}>
            {p.isActive ? 'Disable' : 'Enable'}
          </Button>,
        ])}
      />
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  type = 'text',
  className,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: string;
  className?: string;
}) {
  return (
    <div className={className}>
      <Label className="text-xs">{label}</Label>
      <Input type={type} value={value} onChange={(e) => onChange(e.target.value)} className="mt-1" />
    </div>
  );
}

function StatusBadge({ active, label }: { active: boolean; label: string }) {
  return <Badge variant={active ? 'success' : 'warning'}>{label}</Badge>;
}

function Table({ headers, rows }: { headers: string[]; rows: React.ReactNode[][] }) {
  return (
    <div className="overflow-hidden rounded-md border border-border">
      <table className="w-full text-sm">
        <thead className="bg-card text-xs text-muted-foreground">
          <tr>
            {headers.map((h) => (
              <th key={h} className="border-b border-border px-3 py-2 text-left font-medium">{h}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr><td colSpan={headers.length} className="px-3 py-10 text-center text-xs text-muted-foreground">No records.</td></tr>
          ) : rows.map((row, i) => (
            <tr key={i} className="border-b border-border/50 last:border-0 hover:bg-accent/20">
              {row.map((cell, j) => (
                <td key={j} className="px-3 py-2 align-middle">{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function formatDate(value?: string | null): string {
  if (!value) return '-';
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}
