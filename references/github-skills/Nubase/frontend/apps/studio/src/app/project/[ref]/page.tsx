'use client';

import { useEffect, useState } from 'react';
import Link from 'next/link';
import { Button, Card, CardHeader, CardTitle, CardDescription, CardContent } from '@nubase/ui';
import { Table2, Terminal, Users, HardDrive, Copy, Check } from 'lucide-react';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { API_BASE } from '@/lib/api';
import { useProjectRef } from '@/lib/route-params';

const QUICK_LINKS = [
  { label: 'Browse tables', href: 'editor', icon: Table2 },
  { label: 'Run SQL', href: 'sql', icon: Terminal },
  { label: 'Manage users', href: 'auth', icon: Users },
  { label: 'Browse storage', href: 'storage', icon: HardDrive },
];

export default function ProjectHome({ params }: { params: { ref: string } }) {
  const project = useSession((s) => s.project);
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  const name = project?.name ?? projectRef;

  // In production the Studio bundle is built with NEXT_PUBLIC_NUBASE_API_URL="" so API calls are
  // same-origin relative — which makes API_BASE an empty string. Show the real public origin instead.
  const [origin, setOrigin] = useState('');
  useEffect(() => {
    if (typeof window !== 'undefined') setOrigin(window.location.origin);
  }, []);
  const apiUrl = API_BASE || origin;

  if (!ready) {
    return (
      <div className="space-y-6 p-8">
        <header>
          <p className="text-xs uppercase tracking-wide text-muted-foreground">Project</p>
          <h1 className="text-2xl font-semibold tracking-tight">{name}</h1>
        </header>
        <NotProvisioned projectRef={projectRef} initStatus={project?.initStatus} />
      </div>
    );
  }

  return (
    <div className="space-y-6 p-8">
      <header>
        <p className="text-xs uppercase tracking-wide text-muted-foreground">Project</p>
        <h1 className="text-2xl font-semibold tracking-tight">{name}</h1>
      </header>

      <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {QUICK_LINKS.map((q) => {
          const Icon = q.icon;
          return (
            <Link key={q.href} href={`/project/${projectRef}/${q.href}`}>
              <Card className="h-full transition hover:border-foreground/30">
                <CardContent className="flex flex-col gap-2 p-5">
                  <Icon className="h-5 w-5 text-muted-foreground" />
                  <span className="text-sm font-medium">{q.label}</span>
                </CardContent>
              </Card>
            </Link>
          );
        })}
      </section>

      <section className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Connection</CardTitle>
            <CardDescription>Use this API URL and key in your client.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3 text-xs">
            <CopyField label="URL" value={apiUrl} mono />
            <CopyField label="service_role key" value={project?.apikey ?? ''} mono masked />
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Recent activity</CardTitle>
            <CardDescription>Queries, edits and migrations across this project.</CardDescription>
          </CardHeader>
          <CardContent className="text-sm text-muted-foreground">No activity yet.</CardContent>
        </Card>
      </section>
    </div>
  );
}

/** A label + value row with a copy-to-clipboard button. `masked` shows a truncated preview. */
function CopyField({
  label,
  value,
  mono,
  masked,
}: {
  label: string;
  value: string;
  mono?: boolean;
  masked?: boolean;
}) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard blocked (e.g. insecure context) — silently ignore */
    }
  }

  const display = !value ? '—' : masked ? `${value.slice(0, 16)}…${value.slice(-6)}` : value;

  return (
    <div>
      <p className="text-muted-foreground">{label}</p>
      <div className="flex items-center gap-2">
        <p className={`min-w-0 flex-1 truncate rounded-md bg-muted px-3 py-2 ${mono ? 'font-mono' : ''}`}>
          {display}
        </p>
        <Button
          size="icon"
          variant="ghost"
          onClick={copy}
          disabled={!value}
          aria-label={`Copy ${label}`}
        >
          {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
        </Button>
      </div>
    </div>
  );
}
