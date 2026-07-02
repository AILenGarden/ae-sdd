'use client';

import Link from 'next/link';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  ChevronLeft,
  Folder,
  File as FileIcon,
  Upload,
  RefreshCw,
  Trash2,
  Download,
  Search,
} from 'lucide-react';
import { Button, Input, Badge, Card, CardContent, cn } from '@nubase/ui';
import { API_BASE, type ApiError } from '@/lib/api';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { useProjectRef, useRouteSegmentAfter } from '@/lib/route-params';

interface StorageObject {
  name: string;
  id?: string | null;
  updated_at?: string | null;
  created_at?: string | null;
  last_accessed_at?: string | null;
  metadata?: Record<string, unknown> | null;
}

const PAGE_SIZE = 200;

export default function BucketBrowserPage({
  params,
}: {
  params: { ref: string; bucket: string };
}) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const bucketId = useRouteSegmentAfter('storage', params.bucket);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <BucketBrowserInner projectRef={projectRef} bucketId={bucketId} />;
}

function BucketBrowserInner({ projectRef, bucketId }: { projectRef: string; bucketId: string }) {
  const { project } = useSession();
  const apikey = project!.apikey;

  /** Current path segments inside the bucket. [] = root, ['foo','bar'] = foo/bar/. */
  const [path, setPath] = useState<string[]>([]);
  const [objects, setObjects] = useState<StorageObject[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const prefix = path.length > 0 ? path.join('/') + '/' : '';

  const load = useCallback(
    async (forSearch?: string) => {
      setLoading(true);
      setError(null);
      try {
        const res = await fetch(
          `${API_BASE}/storage/v1/object/list/${encodeURIComponent(bucketId)}`,
          {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', apikey, Authorization: `Bearer ${apikey}` },
            body: JSON.stringify({
              prefix,
              limit: PAGE_SIZE,
              offset: 0,
              search: forSearch ?? undefined,
              sortBy: { column: 'name', order: 'asc' },
            }),
          }
        );
        if (!res.ok) {
          const body = await res.text().catch(() => res.statusText);
          throw new Error(`${res.status} ${body}`);
        }
        const data = (await res.json()) as StorageObject[];
        setObjects(Array.isArray(data) ? data : []);
      } catch (err) {
        setError((err as Error).message ?? 'Failed to list objects.');
      } finally {
        setLoading(false);
      }
    },
    [apikey, bucketId, prefix]
  );

  useEffect(() => {
    load();
  }, [load]);

  /** Split objects into folder placeholders (id === null) and real files. */
  const folders: StorageObject[] = [];
  const files: StorageObject[] = [];
  for (const o of objects) {
    if (!o.id) folders.push(o);
    else files.push(o);
  }

  async function uploadFile(file: File) {
    setUploading(true);
    setError(null);
    try {
      const targetPath = (prefix + file.name).replace(/^\/+/, '');
      const res = await fetch(
        `${API_BASE}/storage/v1/object/${encodeURIComponent(bucketId)}/${encodePath(targetPath)}`,
        {
          method: 'POST',
          headers: {
            apikey,
            Authorization: `Bearer ${apikey}`,
            'Content-Type': file.type || 'application/octet-stream',
            'x-upsert': 'true',
          },
          body: file,
        }
      );
      if (!res.ok) {
        const body = await res.text().catch(() => res.statusText);
        throw new Error(`Upload ${res.status}: ${body}`);
      }
      await load();
    } catch (err) {
      setError((err as Error).message ?? 'Upload failed.');
    } finally {
      setUploading(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  }

  async function deleteFile(name: string) {
    if (!confirm(`Delete ${name}? This cannot be undone.`)) return;
    try {
      const targetPath = (prefix + name).replace(/^\/+/, '');
      const res = await fetch(
        `${API_BASE}/storage/v1/object/${encodeURIComponent(bucketId)}/${encodePath(targetPath)}`,
        { method: 'DELETE', headers: { apikey, Authorization: `Bearer ${apikey}` } }
      );
      if (!res.ok) throw new Error(`${res.status} ${await res.text().catch(() => res.statusText)}`);
      await load();
    } catch (err) {
      setError((err as Error).message ?? 'Delete failed.');
    }
  }

  function enterFolder(folderName: string) {
    setPath((p) => [...p, folderName.replace(/\/+$/, '')]);
  }

  function downloadHref(name: string): string {
    // Object keys are stored percent-encoded, so the list API returns `name`
    // ALREADY URL-encoded — it is the URL path as-is. Re-encoding it would
    // double-encode (e.g. %E6 -> %25E6) and trip Spring's StrictHttpFirewall (403).
    const targetPath = (prefix + name).replace(/^\/+/, '');
    const filename = safeDecode(targetPath.split('/').pop() ?? name);
    return `${API_BASE}/storage/v1/object/${encodeURIComponent(bucketId)}/${targetPath}?download=${encodeURIComponent(filename)}&apikey=${encodeURIComponent(apikey)}`;
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-2 text-sm">
          <Link href={`/project/${projectRef}/storage`}>
            <Button size="icon" variant="ghost" aria-label="Back to buckets">
              <ChevronLeft className="h-4 w-4" />
            </Button>
          </Link>
          <h2 className="font-semibold">{bucketId}</h2>
          <Badge variant="outline">{objects.length} items</Badge>
        </div>
        <div className="flex items-center gap-2">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              load(search.trim() || undefined);
            }}
            className="relative"
          >
            <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Search name…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="h-8 w-56 pl-7 text-xs"
            />
          </form>
          <Button size="sm" variant="outline" onClick={() => load()}>
            <RefreshCw className="h-3.5 w-3.5" /> Refresh
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            hidden
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) uploadFile(f);
            }}
          />
          <Button size="sm" onClick={() => fileInputRef.current?.click()} disabled={uploading}>
            <Upload className="h-3.5 w-3.5" /> {uploading ? 'Uploading…' : 'Upload'}
          </Button>
        </div>
      </header>

      <div className="flex items-center gap-1 border-b border-border bg-card/40 px-4 py-1.5 text-xs">
        <button
          onClick={() => setPath([])}
          className={cn('hover:underline', path.length === 0 && 'font-medium')}
        >
          /
        </button>
        {path.map((seg, i) => (
          <span key={i} className="flex items-center gap-1">
            <span className="text-muted-foreground">/</span>
            <button
              onClick={() => setPath((p) => p.slice(0, i + 1))}
              className={cn('hover:underline', i === path.length - 1 && 'font-medium')}
            >
              {seg}
            </button>
          </span>
        ))}
      </div>

      <div className="flex-1 overflow-auto">
        {error ? (
          <Card className="m-4">
            <CardContent className="p-4 text-sm text-destructive">{error}</CardContent>
          </Card>
        ) : loading ? (
          <p className="p-4 text-sm text-muted-foreground">Loading…</p>
        ) : objects.length === 0 ? (
          <Card className="m-4">
            <CardContent className="flex flex-col items-center gap-2 py-12 text-center">
              <FileIcon className="h-7 w-7 text-muted-foreground" />
              <p className="text-sm text-muted-foreground">
                {prefix ? 'Folder is empty.' : 'Bucket is empty. Upload a file to get started.'}
              </p>
            </CardContent>
          </Card>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-card text-xs text-muted-foreground">
              <tr className="border-b border-border">
                <th className="px-4 py-2 text-left font-medium">Name</th>
                <th className="px-4 py-2 text-left font-medium">Size</th>
                <th className="px-4 py-2 text-left font-medium">Modified</th>
                <th className="px-4 py-2" />
              </tr>
            </thead>
            <tbody>
              {folders.map((f) => (
                <tr
                  key={`d:${f.name}`}
                  onClick={() => enterFolder(f.name)}
                  className="cursor-pointer border-b border-border/40 hover:bg-accent/30"
                >
                  <td className="px-4 py-2">
                    <span className="inline-flex items-center gap-2">
                      <Folder className="h-3.5 w-3.5 text-muted-foreground" />
                      {f.name}
                    </span>
                  </td>
                  <td className="px-4 py-2 text-xs text-muted-foreground">—</td>
                  <td className="px-4 py-2 text-xs text-muted-foreground">—</td>
                  <td className="px-4 py-2 text-right" />
                </tr>
              ))}
              {files.map((f) => {
                const size = Number((f.metadata as Record<string, unknown>)?.size ?? 0);
                return (
                  <tr key={`f:${f.name}`} className="border-b border-border/40 hover:bg-accent/30">
                    <td className="px-4 py-2">
                      <span className="inline-flex items-center gap-2">
                        <FileIcon className="h-3.5 w-3.5 text-muted-foreground" />
                        {f.name}
                      </span>
                    </td>
                    <td className="px-4 py-2 text-xs text-muted-foreground">{size ? formatBytes(size) : '—'}</td>
                    <td className="px-4 py-2 text-xs text-muted-foreground">{formatDate(f.updated_at)}</td>
                    <td className="px-4 py-2 text-right">
                      <a href={downloadHref(f.name)} target="_blank" rel="noreferrer">
                        <Button size="icon" variant="ghost" aria-label="Download">
                          <Download className="h-3.5 w-3.5" />
                        </Button>
                      </a>
                      <button
                        onClick={() => deleteFile(f.name)}
                        className="rounded-md p-1.5 text-muted-foreground hover:bg-destructive/15 hover:text-destructive"
                        aria-label="Delete"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function encodePath(p: string): string {
  return p.split('/').map(encodeURIComponent).join('/');
}

/** Decode a percent-encoded string for display, tolerating malformed input. */
function safeDecode(s: string): string {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatDate(s?: string | null): string {
  if (!s) return '—';
  try {
    return new Date(s).toLocaleString();
  } catch {
    return s;
  }
}
