'use client';

import Link from 'next/link';
import { Users, KeyRound, Building2, Mail, Settings as SettingsIcon } from 'lucide-react';

type AuthTab = 'users' | 'providers' | 'sso' | 'templates' | 'settings';

/**
 * Horizontal sub-nav shared by the pages under /project/[ref]/auth/*.
 * The caller passes its own {@code active} tab name (mirrors the Memory section's sub-nav).
 */
export function AuthSubNav({
  projectRef,
  active,
}: {
  projectRef: string;
  active: AuthTab;
}) {
  const items: Array<{
    key: AuthTab;
    label: string;
    href: string;
    icon: React.ComponentType<{ className?: string }>;
  }> = [
    { key: 'users', label: 'Users', href: `/project/${projectRef}/auth`, icon: Users },
    { key: 'providers', label: 'Providers', href: `/project/${projectRef}/auth/providers`, icon: KeyRound },
    { key: 'sso', label: 'SSO', href: `/project/${projectRef}/auth/sso`, icon: Building2 },
    { key: 'templates', label: 'Email Templates', href: `/project/${projectRef}/auth/templates`, icon: Mail },
    { key: 'settings', label: 'Settings', href: `/project/${projectRef}/auth/settings`, icon: SettingsIcon },
  ];
  return (
    <nav className="border-b border-border bg-muted/10 px-4">
      <ul className="-mb-px flex gap-0.5 text-xs">
        {items.map((it) => {
          const Icon = it.icon;
          const isActive = it.key === active;
          return (
            <li key={it.key}>
              <Link
                href={it.href}
                aria-current={isActive ? 'page' : undefined}
                className={
                  'inline-flex items-center gap-1.5 rounded-t-md border-b-2 px-3 py-2.5 transition-colors ' +
                  (isActive
                    ? 'border-foreground font-medium text-foreground'
                    : 'border-transparent text-muted-foreground hover:bg-muted/50 hover:text-foreground')
                }
              >
                <Icon className="h-3.5 w-3.5" />
                {it.label}
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
