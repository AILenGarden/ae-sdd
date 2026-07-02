'use client';

import { useCallback, useEffect, useState } from 'react';
import { RefreshCw, Save, RotateCcw, XCircle, KeyRound } from 'lucide-react';
import { Button, Card, CardContent } from '@nubase/ui';
import { apiFetch, type ApiError } from '@/lib/api';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { AuthSubNav } from '../_components/sub-nav';
import { SectionCard, Row, BoolInput, TextInput } from '../_components/form-bits';
import { useProjectRef } from '@/lib/route-params';
import type { OAuthProperties, OAuthProviderConfig } from '@/lib/auth-types';

/** OAuth providers Nubase ships backend implementations for. */
const KNOWN_PROVIDERS: Array<{ key: string; label: string; scope: string }> = [
  { key: 'google', label: 'Google', scope: 'openid profile email' },
  { key: 'github', label: 'GitHub', scope: 'read:user user:email' },
  { key: 'wechat', label: 'WeChat', scope: 'snsapi_login' },
];

function emptyProps(): OAuthProperties {
  return { providers: {}, emailConfirmationRequired: null };
}

export default function AuthProvidersPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <ProvidersInner projectRef={projectRef} />;
}

function ProvidersInner({ projectRef }: { projectRef: string }) {
  const { project } = useSession();
  const apikey = project!.apikey;

  const [config, setConfig] = useState<OAuthProperties | null>(null);
  const [draft, setDraft] = useState<OAuthProperties | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const c = await apiFetch<OAuthProperties | null>('/auth/v1/admin/oauth', { apikey });
      const normalized: OAuthProperties = {
        providers: c?.providers ?? {},
        emailConfirmationRequired: c?.emailConfirmationRequired ?? null,
      };
      setConfig(normalized);
      setDraft(structuredClone(normalized));
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load OAuth providers.');
    } finally {
      setLoading(false);
    }
  }, [apikey]);

  useEffect(() => {
    load();
  }, [load]);

  const dirty = config && draft ? JSON.stringify(config) !== JSON.stringify(draft) : false;

  const get = (name: string): OAuthProviderConfig => draft?.providers[name] ?? { enabled: false };

  const patchProvider = (name: string, fn: (p: OAuthProviderConfig) => void) => {
    setDraft((cur) => {
      if (!cur) return cur;
      const next = structuredClone(cur);
      const p = next.providers[name] ?? { enabled: false };
      fn(p);
      next.providers[name] = p;
      return next;
    });
  };

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    setResult(null);
    try {
      await apiFetch('/auth/v1/admin/oauth', { method: 'PUT', body: draft, apikey });
      setResult({ ok: true, message: 'Saved. Takes effect on the next request.' });
      await load();
    } catch (err) {
      setResult({ ok: false, message: (err as ApiError).message ?? 'Save failed.' });
    } finally {
      setSaving(false);
    }
  };

  const revert = () => {
    if (config) setDraft(structuredClone(config));
    setResult(null);
  };

  return (
    <div className="flex h-full flex-col">
      <AuthSubNav projectRef={projectRef} active="providers" />

      <div className="flex-1 overflow-auto">
        <div className="max-w-4xl space-y-6 p-6">
          <header className="flex items-start justify-between gap-4">
            <div>
              <h1 className="text-lg font-semibold">OAuth providers</h1>
              <p className="text-xs text-muted-foreground">
                Social sign-in for the <code className="mx-1 rounded bg-muted/40 px-1">{projectRef}</code> tenant.
                The callback URL is built automatically as
                <code className="mx-1 rounded bg-muted/40 px-1">{`https://{tenant-domain}/auth/v1/callback`}</code>;
                register that as the redirect URI in each provider&apos;s console.
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              {dirty && (
                <Button size="sm" variant="outline" onClick={revert} disabled={saving}>
                  <RotateCcw className="h-3.5 w-3.5" /> Discard
                </Button>
              )}
              <Button size="sm" variant="brand" onClick={save} disabled={!dirty || saving}>
                <Save className={'h-3.5 w-3.5 ' + (saving ? 'animate-pulse' : '')} />
                {saving ? 'Saving…' : 'Save'}
              </Button>
              <Button size="sm" variant="outline" onClick={load} disabled={loading || saving}>
                <RefreshCw className={'h-3.5 w-3.5 ' + (loading ? 'animate-spin' : '')} /> Refresh
              </Button>
            </div>
          </header>

          {result && (
            <p className={'text-xs ' + (result.ok ? 'text-emerald-500' : 'text-destructive')}>{result.message}</p>
          )}
          {error && (
            <Card>
              <CardContent className="flex items-center gap-2 p-4 text-sm text-destructive">
                <XCircle className="h-4 w-4" /> {error}
              </CardContent>
            </Card>
          )}
          {loading && !draft && <p className="py-8 text-center text-sm text-muted-foreground">Loading…</p>}

          {draft &&
            KNOWN_PROVIDERS.map((prov) => {
              const cfg = get(prov.key);
              return (
                <SectionCard
                  key={prov.key}
                  icon={KeyRound}
                  title={prov.label}
                  description={cfg.enabled ? 'Enabled' : 'Disabled — toggle on and add credentials to use.'}
                >
                  <Row label="Enabled">
                    <BoolInput value={cfg.enabled} onChange={(v) => patchProvider(prov.key, (p) => (p.enabled = v))} />
                  </Row>
                  <Row label="Client ID">
                    <TextInput
                      value={cfg.clientId ?? ''}
                      onChange={(v) => patchProvider(prov.key, (p) => (p.clientId = v))}
                      placeholder="client id"
                    />
                  </Row>
                  <Row label="Client Secret">
                    <TextInput
                      value={cfg.clientSecret ?? ''}
                      onChange={(v) => patchProvider(prov.key, (p) => (p.clientSecret = v))}
                      placeholder="••••"
                      type="password"
                    />
                  </Row>
                  <Row label="Scope">
                    <TextInput
                      value={cfg.scope ?? ''}
                      onChange={(v) => patchProvider(prov.key, (p) => (p.scope = v))}
                      placeholder={prov.scope}
                    />
                  </Row>
                  <Row label="Redirect URI (optional)">
                    <TextInput
                      value={cfg.redirectUri ?? ''}
                      onChange={(v) => patchProvider(prov.key, (p) => (p.redirectUri = v))}
                      placeholder="leave blank to auto-build"
                    />
                  </Row>
                </SectionCard>
              );
            })}
        </div>
      </div>
    </div>
  );
}
