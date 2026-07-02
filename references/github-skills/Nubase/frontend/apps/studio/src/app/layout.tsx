import type { Metadata } from 'next';
import { Providers } from './providers';
import '@nubase/ui/styles.css';
import './globals.css';

export const metadata: Metadata = {
  title: 'Nubase Studio',
  description: 'Manage your nubase projects.',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="min-h-screen bg-background font-sans antialiased">
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
