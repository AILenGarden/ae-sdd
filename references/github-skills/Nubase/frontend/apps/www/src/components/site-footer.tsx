import Link from 'next/link';
import { Github, Twitter } from 'lucide-react';
import type { Dict } from '@/lib/i18n';

// Hrefs stay fixed; column titles + link labels come from the dictionary, by index.
const COL_HREFS: string[][] = [
  ['/features', '/compare', '/docs'],
  ['/docs/getting-started', '/docs/concepts', '/docs/memory'],
  ['/blog', '/news', 'https://github.com/OtterMind/Nubase'],
  ['/legal/privacy', '/legal/terms'],
];

export function SiteFooter({ t }: { t: Dict['footer'] }) {
  return (
    <footer className="relative overflow-hidden border-t border-[var(--nb-line)] bg-[var(--nb-bg-2)] text-[var(--nb-ink)]">
      <div className="container relative grid gap-10 py-16 md:grid-cols-[1.5fr_repeat(4,1fr)]">
        <div className="max-w-xs">
          <div className="mb-3 flex items-center gap-2.5">
            <span className="inline-flex items-center justify-center rounded-xl bg-gradient-to-b from-[var(--nb-mint)] to-[var(--nb-mint-deep)] p-1.5">
              <svg viewBox="0 0 320 320" className="h-5 w-5" fill="none" aria-hidden="true">
                <path
                  d="M104 240 V80 L216 240 V80"
                  fill="none"
                  stroke="#07382c"
                  strokeWidth="40"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <circle cx="216" cy="80" r="21" fill="none" stroke="#07382c" strokeWidth="11" />
                <circle cx="104" cy="240" r="12" fill="#ffffff" />
              </svg>
            </span>
            <span className="font-display text-lg font-semibold text-[var(--nb-ink)]">Nubase</span>
          </div>
          <p className="text-sm leading-6 text-[var(--nb-dim)]">{t.tagline}</p>
          <div className="mt-5 flex gap-3">
            <Link
              href="https://github.com/OtterMind/Nubase"
              target="_blank"
              rel="noreferrer"
              aria-label="GitHub"
              className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-[var(--nb-line)] bg-[var(--nb-surface)] text-[var(--nb-dim)] transition-colors hover:border-[var(--nb-mint)] hover:text-[var(--nb-mint)]"
            >
              <Github className="h-4 w-4" />
            </Link>
            <Link
              href="https://x.com"
              target="_blank"
              rel="noreferrer"
              aria-label="X / Twitter"
              className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-[var(--nb-line)] bg-[var(--nb-surface)] text-[var(--nb-dim)] transition-colors hover:border-[var(--nb-mint)] hover:text-[var(--nb-mint)]"
            >
              <Twitter className="h-4 w-4" />
            </Link>
          </div>
        </div>

        {t.cols.map((col, ci) => (
          <div key={col.title}>
            <p className="mb-3 text-xs font-semibold uppercase tracking-[0.14em] text-[var(--nb-dim)]">
              {col.title}
            </p>
            <ul className="space-y-2.5">
              {col.links.map((label, li) => (
                <li key={label}>
                  <Link
                    href={COL_HREFS[ci]?.[li] ?? '#'}
                    className="text-sm text-[var(--nb-dim)] transition-colors hover:text-[var(--nb-mint)]"
                  >
                    {label}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      <div className="border-t border-[var(--nb-line)]">
        <div className="container flex flex-col items-center justify-between gap-2 py-6 text-xs text-[var(--nb-dim)] sm:flex-row">
          <p>&copy; {new Date().getFullYear()} Nubase. {t.copyright}</p>
          <p>{t.madeBy}</p>
        </div>
      </div>
    </footer>
  );
}
