'use client';

import Link from 'next/link';
import { Brain, Tag, Settings as SettingsIcon } from 'lucide-react';

/**
 * Horizontal sub-nav shared by the three pages under /project/[ref]/memory/*.
 *
 * <p>Each page passes its own {@code active} tab name. Underlined tab tracks the active page;
 * unhighlighted tabs are simple links. No client-side routing knowledge baked in — the
 * caller decides which tab is current to avoid pathname pattern-matching duplication.
 */
export function MemorySubNav({
  projectRef,
  active,
}: {
  projectRef: string;
  active: 'memories' | 'entities' | 'settings';
}) {
  const items: Array<{
    key: 'memories' | 'entities' | 'settings';
    label: string;
    href: string;
    icon: React.ComponentType<{ className?: string }>;
  }> = [
    { key: 'memories', label: 'Memories', href: `/project/${projectRef}/memory`, icon: Brain },
    { key: 'entities', label: 'Entities', href: `/project/${projectRef}/memory/entities`, icon: Tag },
    { key: 'settings', label: 'Settings', href: `/project/${projectRef}/memory/settings`, icon: SettingsIcon },
  ];
  return (
    <nav className="border-b border-border px-4">
      <ul className="-mb-px flex gap-1 text-xs">
        {items.map((it) => {
          const Icon = it.icon;
          const isActive = it.key === active;
          return (
            <li key={it.key}>
              <Link
                href={it.href}
                className={
                  'inline-flex items-center gap-1.5 border-b-2 px-3 py-2 ' +
                  (isActive
                    ? 'border-foreground text-foreground'
                    : 'border-transparent text-muted-foreground hover:text-foreground')
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
