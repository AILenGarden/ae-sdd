'use client';

import { useCallback, useEffect, useState } from 'react';
import {
  ShieldCheck,
  KeyRound,
  MessageSquare,
  Bot,
  Gauge,
  Link2,
  Lock,
  UserPlus,
  RefreshCw,
  Save,
  RotateCcw,
  XCircle,
} from 'lucide-react';
import { Button, Card, CardContent } from '@nubase/ui';
import { apiFetch, type ApiError } from '@/lib/api';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { AuthSubNav } from '../_components/sub-nav';
import { SectionCard, Row, BoolInput, NumberInput, TextInput, SelectInput } from '../_components/form-bits';
import { useProjectRef } from '@/lib/route-params';
import type { TenantAuthConfig } from '@/lib/auth-types';

export default function AuthSettingsPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <SettingsInner projectRef={projectRef} />;
}

function SettingsInner({ projectRef }: { projectRef: string }) {
  const { project } = useSession();
  const apikey = project!.apikey;

  const [config, setConfig] = useState<TenantAuthConfig | null>(null);
  const [draft, setDraft] = useState<TenantAuthConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const c = await apiFetch<TenantAuthConfig>('/auth/v1/admin/settings/auth', { apikey });
      setConfig(c);
      setDraft(structuredClone(c));
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load auth settings.');
    } finally {
      setLoading(false);
    }
  }, [apikey]);

  useEffect(() => {
    load();
  }, [load]);

  const dirty = config && draft ? JSON.stringify(config) !== JSON.stringify(draft) : false;

  const patch = (fn: (d: TenantAuthConfig) => void) => {
    setDraft((cur) => {
      if (!cur) return cur;
      const next = structuredClone(cur);
      fn(next);
      return next;
    });
  };

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    setResult(null);
    try {
      await apiFetch('/auth/v1/admin/settings/auth', { method: 'PUT', body: draft, apikey });
      setResult({ ok: true, message: 'Saved. Takes effect on the next request.' });
      await load();
    } catch (err) {
      setResult({ ok: false, message: (err as ApiError).message ?? 'Save failed.' });
    } finally {
      setSaving(false);
    }
  };

  const clearOverride = async () => {
    setClearing(true);
    setResult(null);
    try {
      await apiFetch('/auth/v1/admin/settings/auth', { method: 'DELETE', apikey });
      setResult({ ok: true, message: 'Override cleared — reverted to platform defaults.' });
      await load();
    } catch (err) {
      setResult({ ok: false, message: (err as ApiError).message ?? 'Clear failed.' });
    } finally {
      setClearing(false);
    }
  };

  const revert = () => {
    if (config) setDraft(structuredClone(config));
    setResult(null);
  };

  return (
    <div className="flex h-full flex-col">
      <AuthSubNav projectRef={projectRef} active="settings" />

      <div className="flex-1 overflow-auto">
        <div className="max-w-4xl space-y-6 p-6">
          <header className="flex items-start justify-between gap-4">
            <div>
              <h1 className="text-lg font-semibold">Authentication settings</h1>
              <p className="text-xs text-muted-foreground">
                Per-project overrides for the <code className="mx-1 rounded bg-muted/40 px-1">{projectRef}</code>
                tenant. Each value defaults to the platform
                <code className="mx-1 rounded bg-muted/40 px-1">application.yml</code> config and, when changed
                here, is stored on this project and applied on the next request.
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
            <p className={'text-xs ' + (result.ok ? 'text-emerald-500' : 'text-destructive')}>
              {result.message}
            </p>
          )}
          {error && (
            <Card>
              <CardContent className="flex items-center gap-2 p-4 text-sm text-destructive">
                <XCircle className="h-4 w-4" /> {error}
              </CardContent>
            </Card>
          )}
          {loading && !draft && (
            <p className="py-8 text-center text-sm text-muted-foreground">Loading…</p>
          )}

          {draft && (
            <>
              {/* Sign-up & confirmation */}
              <SectionCard
                icon={UserPlus}
                title="Sign-up & confirmation"
                description="Controls whether new users may register and whether email must be confirmed before sign-in."
              >
                <Row label="Disable public sign-up">
                  <BoolInput value={draft.disableSignup} onChange={(v) => patch((d) => (d.disableSignup = v))} />
                </Row>
                <Row label="Require email confirmation">
                  <BoolInput
                    value={draft.emailConfirmationRequired}
                    onChange={(v) => patch((d) => (d.emailConfirmationRequired = v))}
                  />
                </Row>
                <Row label="OTP / magic-link auto-creates user">
                  <BoolInput
                    value={draft.otp.allowAutoSignup}
                    onChange={(v) => patch((d) => (d.otp.allowAutoSignup = v))}
                  />
                </Row>
              </SectionCard>

              {/* MFA */}
              <SectionCard
                icon={ShieldCheck}
                title="Multi-factor authentication (TOTP / phone)"
                description="When enabled, users can enroll factors and upgrade their session to AAL2."
              >
                <Row label="MFA enabled">
                  <BoolInput value={draft.mfa.enabled} onChange={(v) => patch((d) => (d.mfa.enabled = v))} />
                </Row>
                <Row label="Issuer (authenticator label)">
                  <TextInput value={draft.mfa.issuer} onChange={(v) => patch((d) => (d.mfa.issuer = v))} placeholder="Nubase" />
                </Row>
                <Row label="TOTP digits">
                  <NumberInput value={draft.mfa.digits} min={4} max={8} step={1} onChange={(v) => patch((d) => (d.mfa.digits = v))} />
                </Row>
                <Row label="TOTP period (s)">
                  <NumberInput value={draft.mfa.period} min={10} step={5} onChange={(v) => patch((d) => (d.mfa.period = v))} />
                </Row>
                <Row label="Allowed drift (± steps)">
                  <NumberInput value={draft.mfa.allowedDrift} min={0} max={5} step={1} onChange={(v) => patch((d) => (d.mfa.allowedDrift = v))} />
                </Row>
                <Row label="Max enrolled factors">
                  <NumberInput value={draft.mfa.maxEnrolledFactors} min={1} step={1} onChange={(v) => patch((d) => (d.mfa.maxEnrolledFactors = v))} />
                </Row>
                <Row label="Challenge expiry (s)">
                  <NumberInput value={draft.mfa.challengeExpiration} min={30} step={30} onChange={(v) => patch((d) => (d.mfa.challengeExpiration = v))} />
                </Row>
              </SectionCard>

              {/* Passwordless OTP */}
              <SectionCard
                icon={KeyRound}
                title="Passwordless (OTP / magic link)"
                description="Numeric code length and validity window for email/SMS OTP and magic links."
              >
                <Row label="Code length">
                  <NumberInput value={draft.otp.length} min={4} max={10} step={1} onChange={(v) => patch((d) => (d.otp.length = v))} />
                </Row>
                <Row label="Validity (s)">
                  <NumberInput value={draft.otp.expiration} min={60} step={60} onChange={(v) => patch((d) => (d.otp.expiration = v))} />
                </Row>
              </SectionCard>

              {/* Password policy */}
              <SectionCard
                icon={Lock}
                title="Password policy"
                description="Strength requirements enforced on sign-up and password change."
              >
                <Row label="Minimum length">
                  <NumberInput value={draft.password.minLength} min={1} step={1} onChange={(v) => patch((d) => (d.password.minLength = v))} />
                </Row>
                <Row label="Require uppercase">
                  <BoolInput value={draft.password.requireUppercase} onChange={(v) => patch((d) => (d.password.requireUppercase = v))} />
                </Row>
                <Row label="Require lowercase">
                  <BoolInput value={draft.password.requireLowercase} onChange={(v) => patch((d) => (d.password.requireLowercase = v))} />
                </Row>
                <Row label="Require number">
                  <BoolInput value={draft.password.requireNumber} onChange={(v) => patch((d) => (d.password.requireNumber = v))} />
                </Row>
                <Row label="Require special char">
                  <BoolInput value={draft.password.requireSpecial} onChange={(v) => patch((d) => (d.password.requireSpecial = v))} />
                </Row>
                <Row label="Reauth required to change password">
                  <BoolInput
                    value={draft.password.requireReauthentication}
                    onChange={(v) => patch((d) => (d.password.requireReauthentication = v))}
                  />
                </Row>
              </SectionCard>

              {/* SMS provider */}
              <SectionCard
                icon={MessageSquare}
                title="SMS provider (phone OTP)"
                description="Outbound SMS for phone OTP / phone-MFA codes. 'log' just prints the code (dev)."
              >
                <Row label="Enabled">
                  <BoolInput value={draft.sms.enabled} onChange={(v) => patch((d) => (d.sms.enabled = v))} />
                </Row>
                <Row label="Provider">
                  <SelectInput
                    value={draft.sms.provider}
                    options={['log', 'twilio', 'custom']}
                    onChange={(v) => patch((d) => (d.sms.provider = v))}
                  />
                </Row>
                <Row label="Account SID">
                  <TextInput value={draft.sms.accountSid ?? ''} onChange={(v) => patch((d) => (d.sms.accountSid = v))} placeholder="AC…" />
                </Row>
                <Row label="From number">
                  <TextInput value={draft.sms.fromNumber ?? ''} onChange={(v) => patch((d) => (d.sms.fromNumber = v))} placeholder="+1…" />
                </Row>
                <Row label="Auth token">
                  <TextInput value={draft.sms.authToken ?? ''} onChange={(v) => patch((d) => (d.sms.authToken = v))} placeholder="••••" type="password" />
                </Row>
              </SectionCard>

              {/* CAPTCHA */}
              <SectionCard
                icon={Bot}
                title="CAPTCHA"
                description="Bot protection on signup / signin / recover / otp (hCaptcha or Cloudflare Turnstile)."
              >
                <Row label="Enabled">
                  <BoolInput value={draft.captcha.enabled} onChange={(v) => patch((d) => (d.captcha.enabled = v))} />
                </Row>
                <Row label="Provider">
                  <SelectInput
                    value={draft.captcha.provider}
                    options={['hcaptcha', 'turnstile']}
                    onChange={(v) => patch((d) => (d.captcha.provider = v))}
                  />
                </Row>
                <Row label="Secret">
                  <TextInput value={draft.captcha.secret ?? ''} onChange={(v) => patch((d) => (d.captcha.secret = v))} placeholder="••••" type="password" />
                </Row>
              </SectionCard>

              {/* Rate limiting */}
              <SectionCard
                icon={Gauge}
                title="Rate limiting & lockout"
                description="Throttles sensitive endpoints (otp/recover/signup/token) and locks an identity after repeated failed sign-ins."
              >
                <Row label="Enabled">
                  <BoolInput value={draft.rateLimit.enabled} onChange={(v) => patch((d) => (d.rateLimit.enabled = v))} />
                </Row>
                <Row label="Max requests / window">
                  <NumberInput value={draft.rateLimit.maxRequests} min={1} step={1} onChange={(v) => patch((d) => (d.rateLimit.maxRequests = v))} />
                </Row>
                <Row label="Window (s)">
                  <NumberInput value={draft.rateLimit.windowSeconds} min={1} step={10} onChange={(v) => patch((d) => (d.rateLimit.windowSeconds = v))} />
                </Row>
                <Row label="Max failed logins">
                  <NumberInput value={draft.rateLimit.maxFailedLogins} min={1} step={1} onChange={(v) => patch((d) => (d.rateLimit.maxFailedLogins = v))} />
                </Row>
                <Row label="Lockout (s)">
                  <NumberInput value={draft.rateLimit.lockoutSeconds} min={1} step={30} onChange={(v) => patch((d) => (d.rateLimit.lockoutSeconds = v))} />
                </Row>
              </SectionCard>

              {/* Redirect allow-list */}
              <SectionCard
                icon={Link2}
                title="Redirect URLs (open-redirect guard)"
                description="redirect_to is only honoured if it is relative, the tenant's own domain, localhost (if allowed), or matches an allow-list pattern (* within a segment, ** across segments)."
              >
                <Row label="Allow tenant domain">
                  <BoolInput value={draft.redirect.allowTenantDomain} onChange={(v) => patch((d) => (d.redirect.allowTenantDomain = v))} />
                </Row>
                <Row label="Allow localhost">
                  <BoolInput value={draft.redirect.allowLocalhost} onChange={(v) => patch((d) => (d.redirect.allowLocalhost = v))} />
                </Row>
                <Row label="Fallback site URL">
                  <TextInput value={draft.redirect.siteUrl ?? ''} onChange={(v) => patch((d) => (d.redirect.siteUrl = v))} placeholder="https://app.example.com" />
                </Row>
                <Row label="Allow-list (one per line)">
                  <textarea
                    value={(draft.redirect.allowList ?? []).join('\n')}
                    onChange={(e) =>
                      patch((d) => {
                        d.redirect.allowList = e.target.value
                          .split('\n')
                          .map((s) => s.trim())
                          .filter(Boolean);
                      })
                    }
                    rows={3}
                    placeholder={'https://app.example.com/**\nhttps://*.example.com/auth/callback'}
                    className="w-full rounded-md border border-input bg-transparent px-2.5 py-1.5 font-mono text-xs shadow-sm transition-colors hover:border-muted-foreground/40 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                  />
                </Row>
              </SectionCard>

              {/* Footer: clear override */}
              <Card className="border-border/60">
                <CardContent className="flex items-center justify-between gap-4 p-4">
                  <div>
                    <div className="text-sm font-medium">Reset to platform defaults</div>
                    <p className="mt-1 text-xs text-muted-foreground">
                      Removes this project&apos;s override entirely; every setting reverts to the global
                      <code className="mx-1 rounded bg-muted/40 px-1">application.yml</code> config.
                    </p>
                  </div>
                  <Button size="sm" variant="outline" onClick={clearOverride} disabled={clearing || saving}>
                    <RotateCcw className={'h-3.5 w-3.5 ' + (clearing ? 'animate-spin' : '')} />
                    {clearing ? 'Clearing…' : 'Reset override'}
                  </Button>
                </CardContent>
              </Card>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
