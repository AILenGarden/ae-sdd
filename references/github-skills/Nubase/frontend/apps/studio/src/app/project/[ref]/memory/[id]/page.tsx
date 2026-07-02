'use client';

import { useRouter } from 'next/navigation';
import Link from 'next/link';
import { useCallback, useEffect, useState } from 'react';
import {
  ChevronLeft,
  Pencil,
  Save,
  X,
  Trash2,
  History,
  Tag,
  RefreshCw,
  PlusCircle,
  MinusCircle,
  Edit3,
  AlertCircle,
} from 'lucide-react';
import {
  Button,
  Card,
  CardContent,
  Badge,
  Dialog,
  DialogHeader,
  DialogBody,
  DialogFooter,
} from '@nubase/ui';
import { apiFetch, type ApiError } from '@/lib/api';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { useProjectRef, useRouteSegmentAfter } from '@/lib/route-params';
import type { MemoryItem, MemoryHistoryEntry, EntityItem } from '@/lib/mem-types';

type Tab = 'details' | 'history' | 'entities';

export default function MemoryDetailPage({ params }: { params: { ref: string; id: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const memoryId = useRouteSegmentAfter('memory', params.id);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <MemoryDetailInner projectRef={projectRef} memoryId={memoryId} />;
}

function MemoryDetailInner({ projectRef, memoryId }: { projectRef: string; memoryId: string }) {
  const { project } = useSession();
  const apikey = project!.apikey;
  const router = useRouter();

  const [memory, setMemory] = useState<MemoryItem | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [tab, setTab] = useState<Tab>('details');
  const [history, setHistory] = useState<MemoryHistoryEntry[]>([]);
  const [entities, setEntities] = useState<EntityItem[]>([]);

  // edit state
  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState('');
  const [saving, setSaving] = useState(false);

  const [confirmDelete, setConfirmDelete] = useState(false);

  const loadMemory = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const m = await apiFetch<MemoryItem>(`/mem/v1/memories/${memoryId}`, { apikey });
      setMemory(m);
      setEditText(m.memory);
    } catch (err) {
      const apiErr = err as ApiError;
      setMemory(null);
      setError(apiErr.status === 404 ? 'Memory not found.' : apiErr.message);
    } finally {
      setLoading(false);
    }
  }, [apikey, memoryId]);

  const loadHistory = useCallback(async () => {
    try {
      const h = await apiFetch<MemoryHistoryEntry[]>(
        `/mem/v1/memories/${memoryId}/history`, { apikey });
      setHistory(h ?? []);
    } catch {
      setHistory([]);
    }
  }, [apikey, memoryId]);

  const loadEntities = useCallback(async () => {
    try {
      const e = await apiFetch<EntityItem[]>(
        `/mem/v1/memories/${memoryId}/entities`, { apikey });
      setEntities(e ?? []);
    } catch {
      setEntities([]);
    }
  }, [apikey, memoryId]);

  useEffect(() => { loadMemory(); }, [loadMemory]);
  useEffect(() => {
    if (tab === 'history') loadHistory();
    if (tab === 'entities') loadEntities();
  }, [tab, loadHistory, loadEntities]);

  const saveEdit = async () => {
    if (!editText.trim() || editText === memory?.memory) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      await apiFetch(`/mem/v1/memories/${memoryId}`, {
        apikey,
        method: 'PUT',
        body: { memory: editText },
      });
      setEditing(false);
      await loadMemory();
      // Refresh history if it was loaded — the UPDATE event should appear.
      if (tab === 'history') await loadHistory();
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to update memory.');
    } finally {
      setSaving(false);
    }
  };

  const performDelete = async () => {
    setConfirmDelete(false);
    try {
      await apiFetch(`/mem/v1/memories/${memoryId}`, { apikey, method: 'DELETE' });
      router.push(`/project/${projectRef}/memory`);
    } catch (err) {
      setError((err as ApiError).message ?? 'Delete failed.');
    }
  };

  if (loading) {
    return <div className="p-8 text-sm text-muted-foreground">Loading…</div>;
  }
  if (error || !memory) {
    return (
      <div className="flex flex-col items-start gap-4 p-8">
        <Link
          href={`/project/${projectRef}/memory`}
          className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
        >
          <ChevronLeft className="h-3.5 w-3.5" /> Back to memories
        </Link>
        <Card className="w-full max-w-2xl">
          <CardContent className="flex items-center gap-3 p-6">
            <AlertCircle className="h-5 w-5 text-destructive" />
            <p className="text-sm">{error ?? 'Memory not found.'}</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-3">
          <Link
            href={`/project/${projectRef}/memory`}
            className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
          >
            <ChevronLeft className="h-3.5 w-3.5" /> Back
          </Link>
          <span className="font-mono text-xs text-muted-foreground">{memory.id}</span>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" onClick={loadMemory}>
            <RefreshCw className="h-3.5 w-3.5" /> Refresh
          </Button>
          <Button size="sm" variant="destructive" onClick={() => setConfirmDelete(true)}>
            <Trash2 className="h-3.5 w-3.5" /> Delete
          </Button>
        </div>
      </header>

      {/* Tabs */}
      <div className="border-b border-border px-4">
        <nav className="-mb-px flex gap-4 text-xs">
          {(['details', 'history', 'entities'] as Tab[]).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              className={
                'border-b-2 px-1 py-2 capitalize ' +
                (tab === t
                  ? 'border-foreground text-foreground'
                  : 'border-transparent text-muted-foreground hover:text-foreground')
              }
            >
              {t}
              {t === 'history' && history.length > 0 && (
                <Badge variant="outline" className="ml-1.5 px-1 text-[10px]">{history.length}</Badge>
              )}
              {t === 'entities' && entities.length > 0 && (
                <Badge variant="outline" className="ml-1.5 px-1 text-[10px]">{entities.length}</Badge>
              )}
            </button>
          ))}
        </nav>
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-auto p-4">
        {tab === 'details' && (
          <div className="mx-auto max-w-3xl space-y-4">
            {/* Memory text */}
            <Card>
              <CardContent className="space-y-2 p-4">
                <div className="flex items-center justify-between">
                  <span className="text-[10px] uppercase tracking-wider text-muted-foreground">Memory</span>
                  {!editing ? (
                    <Button size="sm" variant="ghost" onClick={() => { setEditText(memory.memory); setEditing(true); }}>
                      <Pencil className="h-3.5 w-3.5" /> Edit
                    </Button>
                  ) : (
                    <div className="flex gap-1">
                      <Button size="sm" variant="ghost" onClick={() => { setEditing(false); setEditText(memory.memory); }} disabled={saving}>
                        <X className="h-3.5 w-3.5" /> Cancel
                      </Button>
                      <Button size="sm" onClick={saveEdit} disabled={saving}>
                        <Save className="h-3.5 w-3.5" /> {saving ? 'Saving…' : 'Save'}
                      </Button>
                    </div>
                  )}
                </div>
                {!editing ? (
                  <p className="whitespace-pre-wrap text-sm">{memory.memory}</p>
                ) : (
                  <textarea
                    value={editText}
                    onChange={(e) => setEditText(e.target.value)}
                    className="min-h-[120px] w-full rounded-md border border-input bg-background p-2 text-sm"
                  />
                )}
              </CardContent>
            </Card>

            {/* Metadata */}
            <Card>
              <CardContent className="grid grid-cols-2 gap-3 p-4 text-xs sm:grid-cols-3">
                <Meta label="User ID" value={memory.userId} mono />
                <Meta label="Agent ID" value={memory.agentId} />
                <Meta label="Run ID" value={memory.runId} />
                <Meta label="Actor" value={memory.actorId} />
                <Meta label="Role" value={memory.role} />
                <Meta label="Created" value={formatDate(memory.createdAt)} />
                <Meta label="Updated" value={formatDate(memory.updatedAt)} />
                {memory.score != null && <Meta label="Score" value={memory.score.toFixed(3)} />}
              </CardContent>
            </Card>

            {/* Custom metadata JSON */}
            {memory.metadata && Object.keys(memory.metadata).length > 0 && (
              <Card>
                <CardContent className="p-4">
                  <div className="mb-2 text-[10px] uppercase tracking-wider text-muted-foreground">
                    Metadata (JSON)
                  </div>
                  <pre className="overflow-x-auto rounded-md bg-muted/40 p-2 text-[11px]">
                    {JSON.stringify(memory.metadata, null, 2)}
                  </pre>
                </CardContent>
              </Card>
            )}
          </div>
        )}

        {tab === 'history' && (
          <div className="mx-auto max-w-3xl space-y-2">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-semibold"><History className="mr-1.5 inline h-3.5 w-3.5" /> Audit trail</h3>
              <Button size="sm" variant="outline" onClick={loadHistory}>
                <RefreshCw className="h-3.5 w-3.5" /> Refresh
              </Button>
            </div>
            {history.length === 0 ? (
              <p className="py-8 text-center text-xs text-muted-foreground">No history yet.</p>
            ) : (
              <ol className="space-y-2">
                {history.map((h) => (
                  <li key={h.id}>
                    <Card>
                      <CardContent className="space-y-1 p-3">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <EventBadge event={h.event} />
                            <span className="text-[10px] text-muted-foreground">{formatDate(h.createdAt)}</span>
                            {h.actorId && (
                              <span className="text-[10px] text-muted-foreground">by {h.actorId}</span>
                            )}
                          </div>
                        </div>
                        {h.event === 'UPDATE' && (
                          <div className="space-y-1 pt-1">
                            <div className="flex items-start gap-2 text-xs">
                              <MinusCircle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-destructive" />
                              <span className="text-muted-foreground line-through">{h.oldValue}</span>
                            </div>
                            <div className="flex items-start gap-2 text-xs">
                              <PlusCircle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-500" />
                              <span>{h.newValue}</span>
                            </div>
                          </div>
                        )}
                        {h.event === 'ADD' && h.newValue && (
                          <p className="pt-1 text-xs">{h.newValue}</p>
                        )}
                        {h.event === 'DELETE' && h.oldValue && (
                          <p className="pt-1 text-xs text-muted-foreground line-through">{h.oldValue}</p>
                        )}
                      </CardContent>
                    </Card>
                  </li>
                ))}
              </ol>
            )}
          </div>
        )}

        {tab === 'entities' && (
          <div className="mx-auto max-w-3xl space-y-2">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-semibold"><Tag className="mr-1.5 inline h-3.5 w-3.5" /> Linked entities</h3>
              <Button size="sm" variant="outline" onClick={loadEntities}>
                <RefreshCw className="h-3.5 w-3.5" /> Refresh
              </Button>
            </div>
            {entities.length === 0 ? (
              <p className="py-8 text-center text-xs text-muted-foreground">
                No entities link to this memory.
              </p>
            ) : (
              <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                {entities.map((e) => (
                  <li key={e.id}>
                    <Card>
                      <CardContent className="flex items-center justify-between p-3">
                        <div className="min-w-0">
                          <div className="truncate text-sm font-medium">{e.text}</div>
                          {e.entityType && (
                            <div className="mt-0.5">
                              <Badge variant="outline" className="text-[10px]">{e.entityType}</Badge>
                            </div>
                          )}
                        </div>
                        <Link
                          href={`/project/${projectRef}/memory/entities`}
                          className="text-[10px] text-muted-foreground hover:text-foreground"
                        >
                          {(e.linkedMemoryIds?.length ?? 0)} link{(e.linkedMemoryIds?.length ?? 0) === 1 ? '' : 's'}
                        </Link>
                      </CardContent>
                    </Card>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>

      {/* Delete confirm */}
      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogHeader>Delete memory?</DialogHeader>
        <DialogBody>
          <p className="text-sm">This will soft-delete the memory and remove its entity links. The action is reversible only by direct DB intervention.</p>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => setConfirmDelete(false)}>Cancel</Button>
          <Button variant="destructive" onClick={performDelete}>Delete</Button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}

function Meta({ label, value, mono }: { label: string; value?: string | number | null; mono?: boolean }) {
  return (
    <div className="space-y-0.5">
      <div className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</div>
      <div className={mono ? 'font-mono text-[11px]' : 'text-xs'}>{value || '—'}</div>
    </div>
  );
}

function EventBadge({ event }: { event: string }) {
  const variant: Record<string, 'default' | 'secondary' | 'warning' | 'outline'> = {
    ADD: 'default',
    UPDATE: 'secondary',
    DELETE: 'warning',
  };
  const Icon =
    event === 'ADD' ? PlusCircle :
    event === 'UPDATE' ? Edit3 :
    event === 'DELETE' ? MinusCircle : Tag;
  return (
    <Badge variant={variant[event] ?? 'outline'} className="gap-1">
      <Icon className="h-3 w-3" /> {event}
    </Badge>
  );
}

function formatDate(s?: string | null): string {
  if (!s) return '—';
  try {
    return new Date(s).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'medium' });
  } catch {
    return s;
  }
}
