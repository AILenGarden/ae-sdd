import type { MetadataRoute } from 'next';

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: 'Nubase',
    short_name: 'Nubase',
    description:
      'Backend services born for AI — Memory, Database, Storage and Auth in one self-hostable platform.',
    start_url: '/',
    display: 'standalone',
    background_color: '#0B1222',
    theme_color: '#0B1222',
    icons: [
      { src: '/icon-192.png', sizes: '192x192', type: 'image/png', purpose: 'any' },
      { src: '/icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'any' },
      { src: '/icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
    ],
  };
}
