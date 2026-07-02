import Link from 'next/link';

const SECTIONS = [
  {
    title: 'Getting started',
    items: [
      { href: '/docs', label: 'Overview' },
      { href: '/docs/concepts', label: 'The eight modules' },
      { href: '/docs/getting-started', label: 'Quickstart' },
    ],
  },
  {
    title: 'Memory',
    items: [
      { href: '/docs/memory', label: 'Overview' },
      { href: '/docs/memory/quickstart', label: 'Quickstart' },
    ],
  },
  {
    title: 'Database · Storage · Auth',
    items: [
      { href: '/docs/auth', label: 'Auth' },
      { href: '/docs/database', label: 'Database & RLS' },
      { href: '/docs/storage', label: 'Storage' },
      { href: '/docs/rest', label: 'REST API' },
    ],
  },
  {
    title: 'Reference',
    items: [
      { href: '/docs/cli', label: 'CLI' },
      { href: '/docs/changelog', label: 'Changelog' },
    ],
  },
];

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="container grid gap-10 py-12 lg:grid-cols-[220px_1fr]">
      <aside className="lg:sticky lg:top-20 lg:h-fit">
        <nav className="space-y-6 text-sm">
          {SECTIONS.map((s) => (
            <div key={s.title}>
              <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {s.title}
              </p>
              <ul className="space-y-1">
                {s.items.map((i) => (
                  <li key={i.href}>
                    <Link href={i.href} className="text-muted-foreground hover:text-foreground">
                      {i.label}
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </nav>
      </aside>
      <article className="prose prose-invert max-w-none">
        {children}
      </article>
    </div>
  );
}
