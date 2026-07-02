'use client';

import { useEffect, useState } from 'react';
import { useTheme } from 'next-themes';
import { Moon, Sun } from 'lucide-react';

export function ThemeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const isDark = mounted && resolvedTheme === 'dark';
  return (
    <button
      type="button"
      aria-label="Toggle light / dark theme"
      onClick={() => setTheme(isDark ? 'light' : 'dark')}
      className="inline-flex h-9 w-9 items-center justify-center rounded-md border border-[var(--nb-line)] bg-[var(--nb-surface)] text-[var(--nb-ink)] transition-colors hover:bg-[var(--nb-surface-2)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--nb-mint)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--nb-bg)]"
    >
      {/* Suppressed until mounted to avoid a hydration mismatch on the icon. */}
      {isDark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
    </button>
  );
}
