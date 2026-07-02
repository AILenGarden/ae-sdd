'use client';

import Link from 'next/link';
import { useCallback, useEffect, useState } from 'react';
import { HardDrive, Globe, Lock, RefreshCw, Plus, ArrowRight } from 'lucide-react';
import {
  Button,
  Card,
  CardContent,
  Badge,
  Input,
  Label,
  Dialog,
  DialogHeader,
  DialogBody,
  DialogFooter,
} from '@nubase/ui';
import { apiFetch, type ApiError } from '@/lib/api';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { useProjectRef } from '@/lib/route-params';

interface Bucket {
  id: string;
  name: string;
  public?: boolean;
  type?: string;
  file_size_limit?: number | null;
  allowed_mime_types?: string[] | null;
  created_at?: string | null;
  updated_at?: string | null;
}

export default function StorageBucketsPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <StorageBucketsInner />;
}

function StorageBucketsInner() {
  const { project } = useSession();
  const apikey = project!.apikey;
  const projectRef = project!.ref;
  const [buckets, setBuckets] = useState<Bucket[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch<Bucket[]>('/storage/v1/bucket', { apikey });
      setBuckets(res ?? []);
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load buckets.');
    } finally {
      setLoading(false);
    }
  }, [apikey]);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-semibold">Buckets</h2>
          <Badge variant="outline">{buckets.length} total</Badge>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" onClick={load}>
            <RefreshCw className="h-3.5 w-3.5" /> Refresh
          </Button>
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus className="h-3.5 w-3.5" /> New bucket
          </Button>
        </div>
      </header>

      <div className="flex-1 overflow-auto p-6">
        {error ? (
          <Card>
            <CardContent className="p-4 text-sm text-destructive">{error}</CardContent>
          </Card>
        ) : loading ? (
          <p className="text-sm text-muted-foreground">Loading buckets…</p>
        ) : buckets.length === 0 ? (
          <Card>
            <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
              <HardDrive className="h-8 w-8 text-muted-foreground" />
              <p className="text-sm text-muted-foreground">No buckets yet.</p>
              <Button size="sm" onClick={() => setCreating(true)}>
                Create your first bucket
              </Button>
            </CardContent>
          </Card>
        ) : (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {buckets.map((b) => (
              <Link key={b.id} href={`/project/${projectRef}/storage/${encodeURIComponent(b.id)}`}>
                <Card className="h-full transition hover:border-foreground/30">
                  <CardContent className="space-y-3 p-5">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <HardDrive className="h-4 w-4 text-muted-foreground" />
                        <span className="font-medium">{b.name}</span>
                      </div>
                      {b.public ? (
                        <Badge variant="success">
                          <Globe className="mr-1 h-3 w-3" /> public
                        </Badge>
                      ) : (
                        <Badge variant="outline">
                          <Lock className="mr-1 h-3 w-3" /> private
                        </Badge>
                      )}
                    </div>
                    <div className="space-y-1 text-xs text-muted-foreground">
                      <p>
                        ID <code className="font-mono">{b.id}</code>
                      </p>
                      {b.file_size_limit ? <p>Max file: {formatBytes(b.file_size_limit)}</p> : null}
                      {b.allowed_mime_types && b.allowed_mime_types.length > 0 ? (
                        <p>
                          MIME: {b.allowed_mime_types.slice(0, 2).join(', ')}
                          {b.allowed_mime_types.length > 2 ? '…' : ''}
                        </p>
                      ) : null}
                    </div>
                    <div className="flex items-center justify-end pt-1 text-xs text-muted-foreground">
                      Browse <ArrowRight className="ml-1 h-3.5 w-3.5" />
                    </div>
                  </CardContent>
                </Card>
              </Link>
            ))}
          </div>
        )}
      </div>

      <CreateBucketDialog
        open={creating}
        apikey={apikey}
        onClose={() => setCreating(false)}
        onCreated={() => {
          setCreating(false);
          load();
        }}
      />
    </div>
  );
}

interface CreateBucketProps {
  open: boolean;
  apikey: string;
  onClose: () => void;
  onCreated: () => void;
}

function CreateBucketDialog({ open, apikey, onClose, onCreated }: CreateBucketProps) {
  const [name, setName] = useState('');
  const [isPublic, setIsPublic] = useState(false);
  const [fileSizeLimit, setFileSizeLimit] = useState('');
  const [allowedMime, setAllowedMime] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setName('');
      setIsPublic(false);
      setFileSizeLimit('');
      setAllowedMime('');
      setError(null);
    }
  }, [open]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const bucketName = normalizeBucketName(name);
    if (!/^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/.test(bucketName)) {
      setError('Bucket name must be 3-63 characters and use lowercase letters, digits, and hyphens.');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const body: Record<string, unknown> = {
        id: bucketName,
        name: bucketName,
        public: isPublic,
      };
      if (fileSizeLimit) {
        const n = Number(fileSizeLimit);
        if (Number.isFinite(n) && n > 0) body.file_size_limit = n;
      }
      const mimes = allowedMime
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
      if (mimes.length > 0) body.allowed_mime_types = mimes;

      await apiFetch('/storage/v1/bucket', { method: 'POST', body, apikey });
      onCreated();
    } catch (err) {
      setError((err as ApiError).message ?? 'Create failed.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose}>
      <DialogHeader title="New bucket" onClose={onClose} />
      <form onSubmit={submit}>
        <DialogBody className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="bucket-name">Name</Label>
            <Input
              id="bucket-name"
              required
              pattern="[a-z0-9][a-z0-9-]{1,61}[a-z0-9]"
              minLength={3}
              maxLength={63}
              value={name}
              onChange={(e) => setName(normalizeBucketName(e.target.value))}
              placeholder="avatars"
              className="font-mono"
              autoFocus
            />
            <p className="text-[10px] text-muted-foreground">
              3-63 characters. Lowercase letters, digits, and hyphens; must start and end with a letter or digit.
            </p>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={isPublic} onChange={(e) => setIsPublic(e.target.checked)} />
            <span>Public bucket (anyone can read objects)</span>
          </label>
          <div className="space-y-1.5">
            <Label htmlFor="bucket-size">File size limit (bytes, optional)</Label>
            <Input
              id="bucket-size"
              type="number"
              min={0}
              value={fileSizeLimit}
              onChange={(e) => setFileSizeLimit(e.target.value)}
              placeholder="5242880"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="bucket-mime">Allowed MIME types (comma-separated, optional)</Label>
            <Input
              id="bucket-mime"
              value={allowedMime}
              onChange={(e) => setAllowedMime(e.target.value)}
              placeholder="image/png, image/jpeg"
            />
          </div>
          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Creating…' : 'Create bucket'}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function normalizeBucketName(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[\s_]+/g, '-')
    .replace(/[^a-z0-9-]/g, '')
    .replace(/-+/g, '-')
    .slice(0, 63);
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
