'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { Copy, ExternalLink, FileBox, RefreshCw, Settings2, Trash2, Upload } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardContent,
  Dialog,
  DialogBody,
  DialogFooter,
  DialogHeader,
  Input,
  Label,
} from '@nubase/ui';
import { apiFetch, API_BASE, type ApiError } from '@/lib/api';
import { isProjectReady, useSession } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { useProjectRef } from '@/lib/route-params';

interface AssetFile {
  path: string;
  contentType?: string | null;
  sizeBytes: number;
  etag?: string | null;
  cacheControl?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
  publicUrl: string;
}

interface AssetSettings {
  defaultCacheControl: string;
  customBaseUrl?: string | null;
  maxFileSizeBytes?: number | null;
  effectiveMaxFileSizeBytes: number;
  updatedAt?: string | null;
}

export default function AssetsPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <AssetsInner />;
}

function AssetsInner() {
  const { project } = useSession();
  const apikey = project!.apikey;
  const [files, setFiles] = useState<AssetFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [uploading, setUploading] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [copiedPath, setCopiedPath] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (search.trim()) params.set('search', search.trim());
      const qs = params.toString();
      const res = await apiFetch<AssetFile[]>(`/assets/admin/v1/files${qs ? `?${qs}` : ''}`, { apikey });
      setFiles(res ?? []);
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load assets.');
    } finally {
      setLoading(false);
    }
  }, [apikey, search]);

  useEffect(() => {
    load();
  }, [load]);

  async function remove(path: string) {
    if (!window.confirm(`Delete asset "${path}"? Cached copies on CDNs may persist until they expire.`)) {
      return;
    }
    try {
      await apiFetch(`/assets/admin/v1/files/${encodeAssetPath(path)}`, { method: 'DELETE', apikey });
      load();
    } catch (err) {
      setError((err as ApiError).message ?? 'Delete failed.');
    }
  }

  async function copyUrl(file: AssetFile) {
    try {
      await navigator.clipboard.writeText(file.publicUrl);
      setCopiedPath(file.path);
      setTimeout(() => setCopiedPath((p) => (p === file.path ? null : p)), 1500);
    } catch {
      /* clipboard unavailable */
    }
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-semibold">Assets</h2>
          <Badge variant="outline">{files.length} total</Badge>
        </div>
        <div className="flex items-center gap-2">
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search path…"
            className="h-8 w-48"
          />
          <Button size="sm" variant="outline" onClick={load}>
            <RefreshCw className="h-3.5 w-3.5" /> Refresh
          </Button>
          <Button size="sm" variant="outline" onClick={() => setSettingsOpen(true)}>
            <Settings2 className="h-3.5 w-3.5" /> Settings
          </Button>
          <Button size="sm" onClick={() => setUploading(true)}>
            <Upload className="h-3.5 w-3.5" /> Upload
          </Button>
        </div>
      </header>

      <div className="flex-1 overflow-auto p-6">
        {error ? (
          <Card>
            <CardContent className="p-4 text-sm text-destructive">{error}</CardContent>
          </Card>
        ) : loading ? (
          <p className="text-sm text-muted-foreground">Loading assets…</p>
        ) : files.length === 0 ? (
          <Card>
            <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
              <FileBox className="h-8 w-8 text-muted-foreground" />
              <p className="text-sm text-muted-foreground">
                No assets yet. Upload static files (images, css, js) and serve them from your
                project&apos;s public CDN endpoint.
              </p>
              <Button size="sm" onClick={() => setUploading(true)}>
                Upload your first asset
              </Button>
            </CardContent>
          </Card>
        ) : (
          <Card>
            <CardContent className="p-0">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-xs text-muted-foreground">
                    <th className="px-4 py-2 font-medium">Path</th>
                    <th className="px-4 py-2 font-medium">Type</th>
                    <th className="px-4 py-2 font-medium">Size</th>
                    <th className="px-4 py-2 font-medium">Cache-Control</th>
                    <th className="px-4 py-2 font-medium">Updated</th>
                    <th className="px-4 py-2" />
                  </tr>
                </thead>
                <tbody>
                  {files.map((f) => (
                    <tr key={f.path} className="border-b border-border/60 last:border-0">
                      <td className="px-4 py-2 font-mono text-xs">{f.path}</td>
                      <td className="px-4 py-2 text-xs text-muted-foreground">{f.contentType ?? '—'}</td>
                      <td className="px-4 py-2 text-xs text-muted-foreground">{formatBytes(f.sizeBytes)}</td>
                      <td className="px-4 py-2 text-xs text-muted-foreground">
                        {f.cacheControl ?? <span className="italic">project default</span>}
                      </td>
                      <td className="px-4 py-2 text-xs text-muted-foreground">
                        {f.updatedAt ? new Date(f.updatedAt).toLocaleString() : '—'}
                      </td>
                      <td className="px-4 py-2">
                        <div className="flex items-center justify-end gap-1">
                          <Button
                            size="sm"
                            variant="ghost"
                            title="Copy public URL"
                            onClick={() => copyUrl(f)}
                          >
                            <Copy className="h-3.5 w-3.5" />
                            {copiedPath === f.path ? ' Copied' : null}
                          </Button>
                          <a href={f.publicUrl} target="_blank" rel="noreferrer">
                            <Button size="sm" variant="ghost" title="Open">
                              <ExternalLink className="h-3.5 w-3.5" />
                            </Button>
                          </a>
                          <Button
                            size="sm"
                            variant="ghost"
                            title="Delete"
                            onClick={() => remove(f.path)}
                          >
                            <Trash2 className="h-3.5 w-3.5 text-destructive" />
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </CardContent>
          </Card>
        )}
      </div>

      <UploadDialog
        open={uploading}
        apikey={apikey}
        onClose={() => setUploading(false)}
        onUploaded={() => {
          setUploading(false);
          load();
        }}
      />
      <SettingsDialog open={settingsOpen} apikey={apikey} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}

function UploadDialog({
  open,
  apikey,
  onClose,
  onUploaded,
}: {
  open: boolean;
  apikey: string;
  onClose: () => void;
  onUploaded: () => void;
}) {
  const fileInput = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<File | null>(null);
  const [path, setPath] = useState('');
  const [cacheControl, setCacheControl] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setFile(null);
      setPath('');
      setCacheControl('');
      setError(null);
      if (fileInput.current) fileInput.current.value = '';
    }
  }, [open]);

  function onPick(picked: File | null) {
    setFile(picked);
    if (picked && !path) {
      setPath(normalizeAssetPath(picked.name));
    }
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!file) {
      setError('Choose a file to upload.');
      return;
    }
    const targetPath = normalizeAssetPath(path || file.name);
    if (!targetPath) {
      setError('Target path is required.');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (cacheControl.trim()) params.set('cacheControl', cacheControl.trim());
      const qs = params.toString();
      const res = await fetch(
        `${API_BASE}/assets/admin/v1/files/${encodeAssetPath(targetPath)}${qs ? `?${qs}` : ''}`,
        {
          method: 'PUT',
          headers: {
            apikey,
            'Content-Type': file.type || 'application/octet-stream',
          },
          body: file,
        },
      );
      if (!res.ok) {
        const message = await res.text().catch(() => res.statusText);
        throw new Error(message);
      }
      onUploaded();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Upload failed.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose}>
      <DialogHeader title="Upload asset" onClose={onClose} />
      <form onSubmit={submit}>
        <DialogBody className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="asset-file">File</Label>
            <Input
              id="asset-file"
              ref={fileInput}
              type="file"
              onChange={(e) => onPick(e.target.files?.[0] ?? null)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="asset-path">Target path</Label>
            <Input
              id="asset-path"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="img/logo.png"
              className="font-mono"
            />
            <p className="text-[10px] text-muted-foreground">
              Served at /assets/v1/&lt;path&gt;. Letters, digits, &apos;.&apos;, &apos;_&apos;,
              &apos;-&apos; and &apos;/&apos; only. Uploading to an existing path overwrites it.
            </p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="asset-cache">Cache-Control (optional)</Label>
            <Input
              id="asset-cache"
              value={cacheControl}
              onChange={(e) => setCacheControl(e.target.value)}
              placeholder="31536000 or public, max-age=31536000, immutable"
            />
            <p className="text-[10px] text-muted-foreground">
              Plain seconds become max-age=N. Leave empty to use the project default.
            </p>
          </div>
          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Uploading…' : 'Upload'}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function SettingsDialog({
  open,
  apikey,
  onClose,
}: {
  open: boolean;
  apikey: string;
  onClose: () => void;
}) {
  const [settings, setSettings] = useState<AssetSettings | null>(null);
  const [defaultCacheControl, setDefaultCacheControl] = useState('');
  const [customBaseUrl, setCustomBaseUrl] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    apiFetch<AssetSettings>('/assets/admin/v1/settings', { apikey })
      .then((s) => {
        setSettings(s);
        setDefaultCacheControl(s.defaultCacheControl ?? '');
        setCustomBaseUrl(s.customBaseUrl ?? '');
      })
      .catch((err) => setError((err as ApiError).message ?? 'Failed to load settings.'));
  }, [open, apikey]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await apiFetch('/assets/admin/v1/settings', {
        method: 'PATCH',
        apikey,
        body: {
          defaultCacheControl,
          customBaseUrl,
        },
      });
      onClose();
    } catch (err) {
      setError((err as ApiError).message ?? 'Save failed.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose}>
      <DialogHeader title="Asset delivery settings" onClose={onClose} />
      <form onSubmit={submit}>
        <DialogBody className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="settings-cache">Default Cache-Control</Label>
            <Input
              id="settings-cache"
              value={defaultCacheControl}
              onChange={(e) => setDefaultCacheControl(e.target.value)}
              placeholder="public, max-age=3600"
            />
            <p className="text-[10px] text-muted-foreground">
              Applied to assets without a per-file override.
            </p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="settings-base">Custom CDN base URL (optional)</Label>
            <Input
              id="settings-base"
              value={customBaseUrl}
              onChange={(e) => setCustomBaseUrl(e.target.value)}
              placeholder="https://cdn.myapp.io"
            />
            <p className="text-[10px] text-muted-foreground">
              Point your own CDN/domain at this project&apos;s assets, and public URLs become
              &lt;base&gt;/&lt;path&gt;. Leave empty to use the platform CDN.
            </p>
          </div>
          {settings ? (
            <p className="text-[10px] text-muted-foreground">
              Max asset size: {formatBytes(settings.effectiveMaxFileSizeBytes)}
            </p>
          ) : null}
          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Saving…' : 'Save'}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function normalizeAssetPath(value: string): string {
  return value
    .trim()
    .replace(/\\/g, '/')
    .replace(/\s+/g, '-')
    .replace(/[^A-Za-z0-9._/-]/g, '')
    .replace(/\/+/g, '/')
    .replace(/^\/+|\/+$/g, '');
}

function encodeAssetPath(path: string): string {
  return path.split('/').map(encodeURIComponent).join('/');
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
