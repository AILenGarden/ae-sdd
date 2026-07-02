'use client';

import Link from 'next/link';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Brain,
  Search,
  RefreshCw,
  Plus,
  Trash2,
  ChevronLeft,
  ChevronRight,
  X,
  Filter,
  Sparkles,
  Database,
  Activity,
  Users as UsersIcon,
} from 'lucide-react';
import {
  Button,
  Input,
  Label,
  Badge,
  Card,
  CardContent,
  Dialog,
  DialogHeader,
  DialogBody,
  DialogFooter,
} from '@nubase/ui';
import { apiFetch, type ApiError } from '@/lib/api';
import { useSession, isProjectReady } from '@/lib/session';
import { NotProvisioned } from '@/components/not-provisioned';
import { MemorySubNav } from './_components/sub-nav';
import { useProjectRef } from '@/lib/route-params';
import type {
  MemoryItem,
  PagedResponse,
  MemoryStats,
  AddMemoryRequest,
  SearchRequest,
} from '@/lib/mem-types';

const PAGE_SIZES = [25, 50, 100];

export default function MemoryListPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <MemoryListInner projectRef={projectRef} />;
}

function MemoryListInner({ projectRef }: { projectRef: string }) {
  const { project } = useSession();
  const apikey = project!.apikey;

  // -------- list / search state --------
  const [items, setItems] = useState<MemoryItem[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(25);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // -------- filter sidebar state --------
  const [filterUserId, setFilterUserId] = useState('');
  const [filterAgentId, setFilterAgentId] = useState('');
  const [filterRunId, setFilterRunId] = useState('');
  const [filtersApplied, setFiltersApplied] = useState({ userId: '', agentId: '', runId: '' });

  // -------- search mode --------
  const [searchInput, setSearchInput] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const inSearchMode = searchQuery.trim().length > 0;

  // -------- bulk select + delete --------
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmDelete, setConfirmDelete] = useState(false);

  // -------- create dialog --------
  const [createOpen, setCreateOpen] = useState(false);

  // -------- stats --------
  const [stats, setStats] = useState<MemoryStats | null>(null);

  const loadStats = useCallback(async () => {
    try {
      const res = await apiFetch<MemoryStats>('/mem/v1/stats', { apikey });
      setStats(res);
    } catch {
      // Stats are best-effort; don't surface to user.
    }
  }, [apikey]);

  const loadList = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      if (inSearchMode) {
        // Search returns a flat array under {results: []} — no pagination.
        const body: SearchRequest = {
          query: searchQuery,
          userId: filtersApplied.userId || undefined,
          agentId: filtersApplied.agentId || undefined,
          runId: filtersApplied.runId || undefined,
          topK: pageSize,
        };
        const res = await apiFetch<{ results: MemoryItem[] }>('/mem/v1/search', {
          apikey,
          method: 'POST',
          body,
        });
        setItems(res.results ?? []);
        setTotal(res.results?.length ?? 0);
      } else {
        const qs = new URLSearchParams({ page: String(page), pageSize: String(pageSize) });
        if (filtersApplied.userId) qs.set('userId', filtersApplied.userId);
        if (filtersApplied.agentId) qs.set('agentId', filtersApplied.agentId);
        if (filtersApplied.runId) qs.set('runId', filtersApplied.runId);
        const res = await apiFetch<PagedResponse<MemoryItem>>(
          `/mem/v1/memories?${qs}`,
          { apikey },
        );
        setItems(res.items ?? []);
        setTotal(res.total ?? 0);
      }
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load memories.');
      setItems([]);
      setTotal(0);
    } finally {
      setLoading(false);
    }
  }, [apikey, page, pageSize, filtersApplied, inSearchMode, searchQuery]);

  useEffect(() => { loadStats(); }, [loadStats]);
  useEffect(() => { loadList(); }, [loadList]);

  const pageCount = inSearchMode ? 1 : Math.max(1, Math.ceil(total / pageSize));

  const applyFilters = () => {
    setFiltersApplied({
      userId: filterUserId.trim(),
      agentId: filterAgentId.trim(),
      runId: filterRunId.trim(),
    });
    setPage(1);
    setSelected(new Set());
  };
  const clearFilters = () => {
    setFilterUserId('');
    setFilterAgentId('');
    setFilterRunId('');
    setFiltersApplied({ userId: '', agentId: '', runId: '' });
    setPage(1);
  };
  const onSearch = () => {
    setSearchQuery(searchInput.trim());
    setPage(1);
    setSelected(new Set());
  };
  const onClearSearch = () => {
    setSearchInput('');
    setSearchQuery('');
    setPage(1);
  };

  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };
  const toggleSelectAll = () => {
    setSelected((prev) => {
      if (prev.size === items.length) return new Set();
      return new Set(items.map((m) => m.id));
    });
  };

  const performBulkDelete = async () => {
    setConfirmDelete(false);
    setLoading(true);
    try {
      await Promise.all(
        Array.from(selected).map((id) =>
          apiFetch(`/mem/v1/memories/${id}`, { apikey, method: 'DELETE' }),
        ),
      );
      setSelected(new Set());
      await loadList();
      await loadStats();
    } catch (err) {
      setError((err as ApiError).message ?? 'Bulk delete failed.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <MemorySubNav projectRef={projectRef} active="memories" />
      <div className="flex min-h-0 flex-1">
      {/* ---------- left filter sidebar ---------- */}
      <aside className="w-64 shrink-0 border-r border-border bg-card/30">
        <div className="border-b border-border px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          <Filter className="mr-1.5 inline h-3 w-3" /> Filters
        </div>
        <div className="space-y-3 p-3">
          <div className="space-y-1">
            <Label className="text-xs">User ID</Label>
            <Input
              value={filterUserId}
              onChange={(e) => setFilterUserId(e.target.value)}
              placeholder="UUID"
              className="h-8 text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-xs">Agent ID</Label>
            <Input
              value={filterAgentId}
              onChange={(e) => setFilterAgentId(e.target.value)}
              placeholder="agent name"
              className="h-8 text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-xs">Run ID</Label>
            <Input
              value={filterRunId}
              onChange={(e) => setFilterRunId(e.target.value)}
              placeholder="conversation id"
              className="h-8 text-xs"
            />
          </div>
          <div className="flex gap-2 pt-2">
            <Button size="sm" onClick={applyFilters} className="flex-1">
              Apply
            </Button>
            <Button size="sm" variant="outline" onClick={clearFilters}>
              Clear
            </Button>
          </div>
        </div>
      </aside>

      {/* ---------- main area ---------- */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Stats strip */}
        <div className="border-b border-border bg-card/20 px-4 py-3">
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-4">
            <StatCard
              icon={Brain}
              label="Memories"
              value={stats?.totalMemories ?? '—'}
            />
            <StatCard
              icon={Database}
              label="Entities"
              value={stats?.totalEntities ?? '—'}
            />
            <StatCard
              icon={Activity}
              label="Activity (24h)"
              value={
                stats
                  ? `${stats.last24h.add}+ / ${stats.last24h.update}~ / ${stats.last24h.delete}-`
                  : '—'
              }
            />
            <StatCard
              icon={UsersIcon}
              label="Top users"
              value={stats?.topUsers?.length ?? 0}
            />
          </div>
        </div>

        {/* Toolbar */}
        <header className="flex items-center justify-between border-b border-border px-4 py-2">
          <div className="flex items-center gap-2">
            <h2 className="text-sm font-semibold">Memories</h2>
            <Badge variant="outline">{inSearchMode ? `${total} matches` : `${total} total`}</Badge>
            {inSearchMode && (
              <Badge variant="secondary" className="gap-1">
                <Sparkles className="h-3 w-3" /> search
              </Badge>
            )}
          </div>
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={searchInput}
                onChange={(e) => setSearchInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') onSearch();
                }}
                placeholder="Semantic + keyword search"
                className="h-8 w-64 pl-7 pr-7 text-xs"
              />
              {searchInput && (
                <button
                  type="button"
                  onClick={onClearSearch}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
            <Button size="sm" variant="outline" onClick={() => { loadList(); loadStats(); }}>
              <RefreshCw className="h-3.5 w-3.5" /> Refresh
            </Button>
            {selected.size > 0 && (
              <Button size="sm" variant="destructive" onClick={() => setConfirmDelete(true)}>
                <Trash2 className="h-3.5 w-3.5" /> Delete ({selected.size})
              </Button>
            )}
            <Button size="sm" onClick={() => setCreateOpen(true)}>
              <Plus className="h-3.5 w-3.5" /> Add
            </Button>
          </div>
        </header>

        {/* Error */}
        {error && (
          <div className="border-b border-destructive/40 bg-destructive/10 px-4 py-2 text-xs text-destructive">
            {error}
          </div>
        )}

        {/* Table */}
        <div className="flex-1 overflow-auto">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-card text-left text-muted-foreground">
              <tr className="border-b border-border">
                <th className="w-8 px-3 py-2">
                  <input
                    type="checkbox"
                    checked={selected.size > 0 && selected.size === items.length}
                    onChange={toggleSelectAll}
                  />
                </th>
                <th className="px-3 py-2">Memory</th>
                <th className="w-48 px-3 py-2">User</th>
                <th className="w-32 px-3 py-2">Agent / Run</th>
                <th className="w-32 px-3 py-2">Created</th>
                {inSearchMode && <th className="w-20 px-3 py-2">Score</th>}
              </tr>
            </thead>
            <tbody>
              {loading && items.length === 0 ? (
                <tr><td colSpan={inSearchMode ? 6 : 5} className="px-3 py-8 text-center text-muted-foreground">Loading…</td></tr>
              ) : items.length === 0 ? (
                <tr><td colSpan={inSearchMode ? 6 : 5} className="px-3 py-8 text-center text-muted-foreground">No memories.</td></tr>
              ) : (
                items.map((m) => (
                  <tr key={m.id} className="border-b border-border/50 hover:bg-accent/30">
                    <td className="px-3 py-2">
                      <input
                        type="checkbox"
                        checked={selected.has(m.id)}
                        onChange={() => toggleSelect(m.id)}
                      />
                    </td>
                    <td className="px-3 py-2">
                      <Link
                        href={`/project/${projectRef}/memory/${m.id}`}
                        className="block max-w-xl truncate text-foreground hover:underline"
                      >
                        {m.memory}
                      </Link>
                    </td>
                    <td className="px-3 py-2 font-mono text-[10px] text-muted-foreground">
                      {m.userId ? truncateUuid(m.userId) : '—'}
                    </td>
                    <td className="px-3 py-2 text-[10px] text-muted-foreground">
                      {m.agentId || '—'}{m.runId ? ` / ${m.runId}` : ''}
                    </td>
                    <td className="px-3 py-2 text-[10px] text-muted-foreground">
                      {formatDate(m.createdAt)}
                    </td>
                    {inSearchMode && (
                      <td className="px-3 py-2 text-[10px] text-muted-foreground">
                        {m.score != null ? m.score.toFixed(3) : '—'}
                      </td>
                    )}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Pagination */}
        {!inSearchMode && (
          <footer className="flex items-center justify-between border-t border-border px-4 py-2 text-xs">
            <div className="flex items-center gap-2">
              <Label className="text-xs">Page size</Label>
              <select
                value={pageSize}
                onChange={(e) => { setPageSize(Number(e.target.value)); setPage(1); }}
                className="h-7 rounded-md border border-input bg-background px-2 text-xs"
              >
                {PAGE_SIZES.map((n) => <option key={n} value={n}>{n}</option>)}
              </select>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-muted-foreground">
                {(page - 1) * pageSize + 1}-{Math.min(page * pageSize, total)} of {total}
              </span>
              <Button size="sm" variant="outline" disabled={page <= 1} onClick={() => setPage(page - 1)}>
                <ChevronLeft className="h-3.5 w-3.5" />
              </Button>
              <span className="px-2">{page} / {pageCount}</span>
              <Button size="sm" variant="outline" disabled={page >= pageCount} onClick={() => setPage(page + 1)}>
                <ChevronRight className="h-3.5 w-3.5" />
              </Button>
            </div>
          </footer>
        )}
      </div>
      </div>

      {/* Bulk delete confirm */}
      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogHeader>Delete selected memories?</DialogHeader>
        <DialogBody>
          <p className="text-sm">This will soft-delete {selected.size} memorie(s). Entity links will be cleaned up.</p>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => setConfirmDelete(false)}>Cancel</Button>
          <Button variant="destructive" onClick={performBulkDelete}>Delete</Button>
        </DialogFooter>
      </Dialog>

      {/* Create dialog */}
      <CreateMemoryDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        apikey={apikey}
        onDone={() => { loadList(); loadStats(); }}
      />
    </div>
  );
}

function StatCard({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: React.ReactNode;
}) {
  return (
    <Card>
      <CardContent className="flex items-center gap-3 p-3">
        <div className="flex h-8 w-8 items-center justify-center rounded-md bg-accent/40">
          <Icon className="h-4 w-4 text-muted-foreground" />
        </div>
        <div className="min-w-0">
          <div className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</div>
          <div className="truncate text-sm font-semibold">{value}</div>
        </div>
      </CardContent>
    </Card>
  );
}

function CreateMemoryDialog({
  open,
  onOpenChange,
  apikey,
  onDone,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  apikey: string;
  onDone: () => void;
}) {
  const [content, setContent] = useState('');
  const [userId, setUserId] = useState('');
  const [agentId, setAgentId] = useState('');
  const [runId, setRunId] = useState('');
  const [infer, setInfer] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const reset = () => {
    setContent(''); setUserId(''); setAgentId(''); setRunId(''); setInfer(true); setErr(null);
  };

  const submit = async () => {
    if (!content.trim()) return;
    if (!userId.trim() && !agentId.trim() && !runId.trim()) {
      setErr('At least one of userId / agentId / runId is required.');
      return;
    }
    setSubmitting(true);
    setErr(null);
    try {
      const body: AddMemoryRequest = {
        messages: [{ role: 'user', content }],
        userId: userId.trim() || undefined,
        agentId: agentId.trim() || undefined,
        runId: runId.trim() || undefined,
        infer,
      };
      await apiFetch('/mem/v1/memories', { apikey, method: 'POST', body });
      reset();
      onOpenChange(false);
      onDone();
    } catch (e) {
      setErr((e as ApiError).message ?? 'Failed to add memory.');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogHeader>Add memory</DialogHeader>
      <DialogBody>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label className="text-xs">Content</Label>
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="What should be remembered?"
              className="min-h-[80px] w-full rounded-md border border-input bg-background p-2 text-sm"
            />
          </div>
          <div className="grid grid-cols-3 gap-2">
            <div className="space-y-1">
              <Label className="text-xs">User ID</Label>
              <Input value={userId} onChange={(e) => setUserId(e.target.value)} className="h-8 text-xs" />
            </div>
            <div className="space-y-1">
              <Label className="text-xs">Agent ID</Label>
              <Input value={agentId} onChange={(e) => setAgentId(e.target.value)} className="h-8 text-xs" />
            </div>
            <div className="space-y-1">
              <Label className="text-xs">Run ID</Label>
              <Input value={runId} onChange={(e) => setRunId(e.target.value)} className="h-8 text-xs" />
            </div>
          </div>
          <label className="flex items-center gap-2 text-xs">
            <input type="checkbox" checked={infer} onChange={(e) => setInfer(e.target.checked)} />
            Use LLM to extract facts (infer=true). Disable to store verbatim.
          </label>
          {err && <p className="text-xs text-destructive">{err}</p>}
        </div>
      </DialogBody>
      <DialogFooter>
        <Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
        <Button onClick={submit} disabled={submitting || !content.trim()}>
          {submitting ? 'Adding…' : 'Add'}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function truncateUuid(u: string): string {
  return u.length > 8 ? u.slice(0, 8) + '…' : u;
}

function formatDate(s?: string | null): string {
  if (!s) return '—';
  try {
    const d = new Date(s);
    return d.toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' });
  } catch {
    return s;
  }
}
