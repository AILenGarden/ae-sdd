'use client';

import { useCallback, useEffect, useState } from 'react';
import {
  Search,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
  UserPlus,
  Ban,
  CheckCircle2,
  Trash2,
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
import { AuthSubNav } from './_components/sub-nav';
import { useProjectRef } from '@/lib/route-params';

interface AuthUser {
  id: string;
  email?: string | null;
  phone?: string | null;
  role?: string | null;
  email_confirmed_at?: string | null;
  last_sign_in_at?: string | null;
  created_at?: string | null;
  banned_until?: string | null;
  user_metadata?: Record<string, unknown> | null;
  app_metadata?: Record<string, unknown> | null;
}

interface ListUsersResponse {
  users: AuthUser[];
  total: number;
  page: number;
  per_page: number;
}

const PER_PAGE = 25;

export default function AuthUsersPage({ params }: { params: { ref: string } }) {
  const { project } = useSession();
  const projectRef = useProjectRef(params.ref);
  const ready = isProjectReady(project);
  if (!ready) {
    return <NotProvisioned projectRef={project?.ref ?? projectRef} initStatus={project?.initStatus} />;
  }
  return <AuthUsersInner />;
}

function AuthUsersInner() {
  const { project } = useSession();
  const [users, setUsers] = useState<AuthUser[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [keyword, setKeyword] = useState('');
  const [keywordInput, setKeywordInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [invitingOpen, setInvitingOpen] = useState(false);
  const [selectedUser, setSelectedUser] = useState<AuthUser | null>(null);

  const load = useCallback(async () => {
    if (!project) return;
    setLoading(true);
    setError(null);
    try {
      const qs = new URLSearchParams({ page: String(page), per_page: String(PER_PAGE) });
      if (keyword) qs.set('keyword', keyword);
      const res = await apiFetch<ListUsersResponse>(`/auth/v1/admin/users?${qs}`, {
        apikey: project.apikey,
      });
      setUsers(res.users ?? []);
      setTotal(res.total ?? 0);
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load users.');
    } finally {
      setLoading(false);
    }
  }, [project, page, keyword]);

  useEffect(() => {
    load();
  }, [load]);

  const pageCount = Math.max(1, Math.ceil(total / PER_PAGE));

  return (
    <div className="flex h-full flex-col">
      <AuthSubNav projectRef={project!.ref} active="users" />
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-semibold">Users</h2>
          <Badge variant="outline">{total} total</Badge>
        </div>
        <div className="flex items-center gap-2">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              setPage(1);
              setKeyword(keywordInput.trim());
            }}
            className="relative"
          >
            <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Search email, phone, role…"
              value={keywordInput}
              onChange={(e) => setKeywordInput(e.target.value)}
              className="h-8 w-72 pl-7 text-xs"
            />
          </form>
          <Button size="sm" variant="outline" onClick={load}>
            <RefreshCw className="h-3.5 w-3.5" /> Refresh
          </Button>
          <Button size="sm" onClick={() => setInvitingOpen(true)}>
            <UserPlus className="h-3.5 w-3.5" /> Invite
          </Button>
        </div>
      </header>

      <div className="flex-1 overflow-auto">
        {error ? (
          <Card className="m-4">
            <CardContent className="p-4 text-sm text-destructive">{error}</CardContent>
          </Card>
        ) : loading ? (
          <p className="p-4 text-sm text-muted-foreground">Loading users…</p>
        ) : users.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">No users.</p>
        ) : (
          <table className="w-full border-collapse text-sm">
            <thead className="sticky top-0 bg-card">
              <tr className="border-b border-border">
                <th className="px-3 py-2 text-left font-medium">User</th>
                <th className="px-3 py-2 text-left font-medium">Role</th>
                <th className="px-3 py-2 text-left font-medium">Last sign in</th>
                <th className="px-3 py-2 text-left font-medium">Created</th>
                <th className="px-3 py-2 text-left font-medium">Status</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr
                  key={u.id}
                  onClick={() => setSelectedUser(u)}
                  className="cursor-pointer border-b border-border/50 hover:bg-accent/30"
                >
                  <td className="px-3 py-2">
                    <div className="font-medium">{u.email ?? u.phone ?? '(no identifier)'}</div>
                    <div className="font-mono text-[10px] text-muted-foreground">{u.id}</div>
                  </td>
                  <td className="px-3 py-2 text-xs text-muted-foreground">{u.role ?? '—'}</td>
                  <td className="px-3 py-2 text-xs text-muted-foreground">{formatDate(u.last_sign_in_at)}</td>
                  <td className="px-3 py-2 text-xs text-muted-foreground">{formatDate(u.created_at)}</td>
                  <td className="px-3 py-2">
                    {u.banned_until ? (
                      <Badge variant="warning">banned</Badge>
                    ) : u.email_confirmed_at ? (
                      <Badge variant="success">confirmed</Badge>
                    ) : (
                      <Badge variant="outline">pending</Badge>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <footer className="flex items-center justify-between border-t border-border px-4 py-2 text-xs text-muted-foreground">
        <span>
          Page {page} of {pageCount}
        </span>
        <div className="flex gap-1">
          <Button size="sm" variant="outline" onClick={() => setPage((p) => Math.max(1, p - 1))} disabled={page <= 1}>
            <ChevronLeft className="h-3.5 w-3.5" /> Prev
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => setPage((p) => Math.min(pageCount, p + 1))}
            disabled={page >= pageCount}
          >
            Next <ChevronRight className="h-3.5 w-3.5" />
          </Button>
        </div>
      </footer>

      <InviteDialog
        open={invitingOpen}
        apikey={project!.apikey}
        onClose={() => setInvitingOpen(false)}
        onInvited={() => {
          setInvitingOpen(false);
          load();
        }}
      />

      <UserDetailsDialog
        user={selectedUser}
        apikey={project!.apikey}
        onClose={() => setSelectedUser(null)}
        onChanged={() => {
          setSelectedUser(null);
          load();
        }}
      />
    </div>
  );
}

interface InviteDialogProps {
  open: boolean;
  apikey: string;
  onClose: () => void;
  onInvited: () => void;
}

function InviteDialog({ open, apikey, onClose, onInvited }: InviteDialogProps) {
  const [email, setEmail] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setEmail('');
      setError(null);
    }
  }, [open]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await apiFetch('/auth/v1/invite', {
        method: 'POST',
        body: { email },
        apikey,
      });
      onInvited();
    } catch (err) {
      setError((err as ApiError).message ?? 'Invite failed.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose}>
      <DialogHeader
        title="Invite user"
        description="An invitation email will be sent with a confirmation link."
        onClose={onClose}
      />
      <form onSubmit={submit}>
        <DialogBody className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="invite-email">Email</Label>
            <Input
              id="invite-email"
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="user@example.com"
              autoFocus
            />
          </div>
          {error ? <p className="text-xs text-destructive">{error}</p> : null}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button type="submit" disabled={submitting}>
            {submitting ? 'Sending…' : 'Send invite'}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

interface UserDetailsDialogProps {
  user: AuthUser | null;
  apikey: string;
  onClose: () => void;
  onChanged: () => void;
}

function UserDetailsDialog({ user, apikey, onClose, onChanged }: UserDetailsDialogProps) {
  const [working, setWorking] = useState<null | 'ban' | 'unban' | 'delete'>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    if (user) {
      setError(null);
      setConfirmDelete(false);
    }
  }, [user]);

  if (!user) return null;

  const banned = Boolean(user.banned_until);

  async function toggleBan() {
    setWorking(banned ? 'unban' : 'ban');
    setError(null);
    try {
      await apiFetch(`/auth/v1/admin/users/${encodeURIComponent(user!.id)}`, {
        method: 'PUT',
        body: { ban_duration: banned ? 'none' : 'permanent' },
        apikey,
      });
      onChanged();
    } catch (err) {
      setError((err as ApiError).message ?? 'Action failed.');
    } finally {
      setWorking(null);
    }
  }

  async function doDelete() {
    setWorking('delete');
    setError(null);
    try {
      await apiFetch(`/auth/v1/admin/users/${encodeURIComponent(user!.id)}`, {
        method: 'DELETE',
        apikey,
      });
      onChanged();
    } catch (err) {
      setError((err as ApiError).message ?? 'Delete failed.');
    } finally {
      setWorking(null);
    }
  }

  return (
    <Dialog open={Boolean(user)} onClose={onClose} size="max-w-lg">
      <DialogHeader
        title={user.email ?? user.phone ?? 'User'}
        description={user.id}
        onClose={onClose}
      />
      <DialogBody className="space-y-3 text-sm">
        <Row label="Role" value={user.role ?? '—'} />
        <Row label="Phone" value={user.phone ?? '—'} />
        <Row label="Email confirmed" value={formatDate(user.email_confirmed_at)} />
        <Row label="Last sign-in" value={formatDate(user.last_sign_in_at)} />
        <Row label="Created" value={formatDate(user.created_at)} />
        <Row label="Banned until" value={user.banned_until ? formatDate(user.banned_until) : '—'} />
        {user.user_metadata && Object.keys(user.user_metadata).length > 0 ? (
          <div>
            <p className="mb-1 text-xs text-muted-foreground">user_metadata</p>
            <pre className="max-h-48 overflow-auto rounded-md border border-border bg-muted p-2 text-[11px]">
              {JSON.stringify(user.user_metadata, null, 2)}
            </pre>
          </div>
        ) : null}
        {error ? <p className="text-xs text-destructive">{error}</p> : null}
      </DialogBody>
      <DialogFooter>
        {confirmDelete ? (
          <>
            <span className="mr-auto text-xs text-destructive">Delete this user permanently?</span>
            <Button variant="outline" size="sm" onClick={() => setConfirmDelete(false)} disabled={working !== null}>
              Cancel
            </Button>
            <Button variant="destructive" size="sm" onClick={doDelete} disabled={working !== null}>
              {working === 'delete' ? 'Deleting…' : 'Yes, delete'}
            </Button>
          </>
        ) : (
          <>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => setConfirmDelete(true)}
              disabled={working !== null}
              className="mr-auto"
            >
              <Trash2 className="h-3.5 w-3.5" /> Delete
            </Button>
            <Button variant="outline" size="sm" onClick={toggleBan} disabled={working !== null}>
              {banned ? (
                <>
                  <CheckCircle2 className="h-3.5 w-3.5" />
                  {working === 'unban' ? 'Unbanning…' : 'Unban'}
                </>
              ) : (
                <>
                  <Ban className="h-3.5 w-3.5" />
                  {working === 'ban' ? 'Banning…' : 'Ban'}
                </>
              )}
            </Button>
            <Button size="sm" onClick={onClose}>
              Close
            </Button>
          </>
        )}
      </DialogFooter>
    </Dialog>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[140px_1fr] gap-3">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-xs">{value}</span>
    </div>
  );
}

function formatDate(s?: string | null): string {
  if (!s) return '—';
  try {
    return new Date(s).toLocaleString();
  } catch {
    return s;
  }
}
