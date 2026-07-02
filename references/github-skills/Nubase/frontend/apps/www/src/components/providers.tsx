'use client';

import { ThemeProvider } from 'next-themes';

// System-followed light/dark with a manual override (persisted by next-themes).
export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
      {children}
    </ThemeProvider>
  );
}
