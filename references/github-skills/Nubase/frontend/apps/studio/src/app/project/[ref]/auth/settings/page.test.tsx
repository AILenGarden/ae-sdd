import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import type { TenantAuthConfig } from '@/lib/auth-types';
import AuthSettingsPage from './page';

// ---- module mocks (hoisted by vitest) ----
vi.mock('@/lib/api', () => ({ apiFetch: vi.fn() }));
vi.mock('@/lib/session', () => ({
  useSession: () => ({ project: { ref: 'demo', apikey: 'KEY', initStatus: 'INITIALIZED' } }),
  isProjectReady: () => true,
}));
vi.mock('@/components/not-provisioned', () => ({ NotProvisioned: () => <div>not provisioned</div> }));
vi.mock('../_components/sub-nav', () => ({ AuthSubNav: () => <nav data-testid="subnav" /> }));
vi.mock('@nubase/ui', () => ({
  Button: (p: any) => <button disabled={p.disabled} onClick={p.onClick}>{p.children}</button>,
  Card: (p: any) => <div>{p.children}</div>,
  CardContent: (p: any) => <div>{p.children}</div>,
}));

import { apiFetch } from '@/lib/api';

const effective: TenantAuthConfig = {
  mfa: { enabled: true, issuer: 'Nubase', digits: 6, period: 30, allowedDrift: 1, maxEnrolledFactors: 10, challengeExpiration: 300 },
  otp: { length: 6, expiration: 3600, allowAutoSignup: true },
  sms: { enabled: false, provider: 'log', accountSid: null, authToken: null, fromNumber: null },
  captcha: { enabled: false, provider: 'hcaptcha', secret: null },
  rateLimit: { enabled: true, maxRequests: 30, windowSeconds: 300, maxFailedLogins: 5, lockoutSeconds: 900 },
  redirect: { allowTenantDomain: true, allowLocalhost: true, siteUrl: null, allowList: [] },
  password: { minLength: 6, requireUppercase: false, requireLowercase: false, requireNumber: false, requireSpecial: false, requireReauthentication: false },
  emailConfirmationRequired: true,
  disableSignup: false,
};

describe('AuthSettingsPage', () => {
  beforeEach(() => {
    vi.mocked(apiFetch).mockReset();
    vi.mocked(apiFetch).mockImplementation((_path: string, opts: any = {}) => {
      if (opts.method === 'PUT' || opts.method === 'DELETE') return Promise.resolve({});
      return Promise.resolve(structuredClone(effective)); // GET
    });
  });

  it('loads the effective config on mount and renders the setting groups', async () => {
    render(<AuthSettingsPage params={{ ref: 'demo' }} />);

    expect(await screen.findByText('Disable public sign-up')).toBeInTheDocument();
    expect(screen.getByText('Multi-factor authentication (TOTP / phone)')).toBeInTheDocument();
    // loaded via the admin endpoint with the project apikey
    expect(apiFetch).toHaveBeenCalledWith('/auth/v1/admin/settings/auth', expect.objectContaining({ apikey: 'KEY' }));
  });

  it('PUTs the updated config when a field is changed and Save is clicked', async () => {
    render(<AuthSettingsPage params={{ ref: 'demo' }} />);
    await screen.findByText('Disable public sign-up');

    // toggle the first boolean (Disable public sign-up) → makes the form dirty
    const checkboxes = screen.getAllByRole('checkbox');
    fireEvent.click(checkboxes[0]);

    const save = screen.getByRole('button', { name: /Save/ });
    expect(save).not.toBeDisabled();
    fireEvent.click(save);

    await waitFor(() => {
      expect(apiFetch).toHaveBeenCalledWith(
        '/auth/v1/admin/settings/auth',
        expect.objectContaining({ method: 'PUT', apikey: 'KEY' }),
      );
    });
    const putCall = vi.mocked(apiFetch).mock.calls.find((c) => (c[1] as any)?.method === 'PUT');
    expect((putCall![1] as any).body.disableSignup).toBe(true);
  });
});
