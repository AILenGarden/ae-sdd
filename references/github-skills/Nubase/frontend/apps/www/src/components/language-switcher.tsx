'use client';

import { Languages } from 'lucide-react';
import { LANG_COOKIE, LANG_NAMES, LANGS, type Lang } from '@/lib/i18n';

export function LanguageSwitcher({ current }: { current: Lang }) {
  function onChange(e: React.ChangeEvent<HTMLSelectElement>) {
    document.cookie = `${LANG_COOKIE}=${e.target.value};path=/;max-age=31536000;samesite=lax`;
    window.location.reload();
  }
  return (
    <label className="relative inline-flex items-center">
      <Languages className="pointer-events-none absolute left-2.5 h-4 w-4 text-[var(--nb-dim)]" aria-hidden />
      <select
        value={current}
        onChange={onChange}
        aria-label="Language"
        className="h-9 cursor-pointer appearance-none rounded-md border border-[var(--nb-line)] bg-[var(--nb-surface)] py-0 pl-8 pr-3 text-sm font-medium text-[var(--nb-ink)] transition-colors hover:bg-[var(--nb-surface-2)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--nb-mint)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--nb-bg)]"
      >
        {LANGS.map((l) => (
          <option key={l} value={l}>
            {LANG_NAMES[l]}
          </option>
        ))}
      </select>
    </label>
  );
}
