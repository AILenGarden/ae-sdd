import { cookies, headers } from 'next/headers';
import { DEFAULT_LANG, LANG_COOKIE, LANGS, type Lang } from '@/lib/i18n';

// Resolve the homepage language (server only): an explicit cookie wins; otherwise
// parse the browser's Accept-Language and pick the first supported tag; else English.
export function getLang(): Lang {
  const cookieLang = cookies().get(LANG_COOKIE)?.value;
  if (cookieLang && (LANGS as readonly string[]).includes(cookieLang)) return cookieLang as Lang;

  const accept = headers().get('accept-language') ?? '';
  for (const part of accept.split(',')) {
    const tag = (part.trim().split(';')[0] ?? '').toLowerCase();
    const base = tag.split('-')[0] ?? '';
    if ((LANGS as readonly string[]).includes(base)) return base as Lang;
  }
  return DEFAULT_LANG;
}
