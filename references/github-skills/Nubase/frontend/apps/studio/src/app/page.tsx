'use client';

import { useEffect } from 'react';
import { useRouter } from 'next/navigation';

// Client-side redirect (works under static export, unlike a server-component redirect()).
// next/navigation resolves this against basePath, so it lands on /studio/projects when
// served by the Java backend, or /projects under `pnpm dev:studio`.
export default function Index() {
  const router = useRouter();
  useEffect(() => {
    router.replace('/projects');
  }, [router]);
  return null;
}
