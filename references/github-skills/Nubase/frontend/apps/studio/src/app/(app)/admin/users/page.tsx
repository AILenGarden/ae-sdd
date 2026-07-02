'use client';

import { useCallback, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { RefreshCw, ShieldCheck, ShieldOff, Trash2, UserX, UserCheck } from 'lucide-react';
import {
  Button,
  Badge,
  Card,
  CardContent,
} from '@nubase/ui';
import { apiFetch, type ApiError } from '@/lib/api';
import { useSession, isSuperAdmin } from '@/lib/session';

interface PlatformUserRow {
  id: string;
  email: string;
  fullName?: string | null;
  role: string;
  isActive: boolean;
  lastSignedInAt?: string | null;
  createdAt?: string | null;
}

export default function PlatformUsersPage() {
  const router = useRouter();
  const { platformKey, user, hasHydrated } = useSession();
  const superAdmin = isSuperAdmin(user);
  const [users, setUsers] = useState<PlatformUserRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!platformKey) return;
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch<PlatformUserRow[]>('/auth/v1/admin/platform/users', {
        apikey: platformKey,
      });
      setUsers(res);
    } catch (err) {
      setError((err as ApiError).message ?? 'Failed to load users.');
    } finally {
      setLoading(false);
    }
  }, [platformKey]);

  useEffect(() => {
    if (!hasHydrated) return;
    if (!platformKey) {
      router.replace('/login');
      return;
    }
    if (!superAdmin) {
      router.replace('/projects');
      return;
    }
    load();
  }, [hasHydrated, platformKey, superAdmin, router, load]);

  async function updateUser(id: string, body: Partial<{ role: string; isActive: boolean }>) {
    if (!platformKey) return;
    setBusy(id);
    try {
      await apiFetch(`/auth/v1/admin/platform/users/${encodeURIComponent(id)}`, {
        method: 'PUT',
        body,
        apikey: platformKey,
      });
      await load();
    } catch (err) {
      setError((err as ApiError).message ?? 'Update failed.');
    } finally {
      setBusy(null);
    }
  }

  async function deleteUser(id: string, email: string) {
    if (!platformKey) return;
    if (!confirm(`Permanently delete platform user ${email}? They will lose access to all projects.`)) return;
    setBusy(id);
    try {
      await apiFetch(`/auth/v1/admin/platform/users/${encodeURIComponent(id)}`, {
        method: 'DELETE',
        apikey: platformKey,
      });
      await load();
    } catch (err) {
      setError((err as ApiError).message ?? 'Delete failed.');
    } finally {
      setBusy(null);
    }
  }

  if (!hasHydrated || !superAdmin) return null;

  return (
    <div className="mx-auto max-w-5xl space-y-6 p-8">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Platform users</h1>
          <p className="text-sm text-muted-foreground">
            Developer accounts that can sign into this Studio. Visible only to super admins.
          </p>
        </div>
        <Button size="sm" variant="outline" onClick={load}>
          <RefreshCw className="h-3.5 w-3.5" /> Refresh
        </Button>
      </header>

      {error ? <p className="text-sm text-destructive">{error}</p> : null}

      <Card>
        <CardContent className="p-0">
          {loading ? (
            <p className="p-4 text-sm text-muted-foreground">Loading…</p>
          ) : users.length === 0 ? (
            <p className="p-4 text-sm text-muted-foreground">No platform users.</p>
          ) : (
            <table className="w-full text-sm">
              <thead className="border-b border-border text-xs text-muted-foreground">
                <tr>
                  <th className="px-4 py-2 text-left font-medium">User</th>
                  <th className="px-4 py-2 text-left font-medium">Role</th>
                  <th className="px-4 py-2 text-left font-medium">Status</th>
                  <th className="px-4 py-2 text-left font-medium">Last sign-in</th>
                  <th className="px-4 py-2 text-right font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {users.map((u) => {
                  const isYou = user?.id === u.id;
                  const isAdmin = u.role?.toLowerCase() === 'super_admin';
                  return (
                    <tr key={u.id} className="border-b border-border/40 last:border-b-0">
                      <td className="px-4 py-2.5">
                        <div className="font-medium">
                          {u.email}
                          {isYou ? <span className="ml-2 text-xs text-muted-foreground">(you)</span> : null}
                        </div>
                        {u.fullName ? <div className="text-xs text-muted-foreground">{u.fullName}</div> : null}
                        <div className="font-mono text-[10px] text-muted-foreground">{u.id}</div>
                      </td>
                      <td className="px-4 py-2.5">
                        {isAdmin ? (
                          <Badge variant="success">super admin</Badge>
                        ) : (
                          <Badge variant="outline">user</Badge>
                        )}
                      </td>
                      <td className="px-4 py-2.5">
                        {u.isActive ? (
                          <Badge variant="outline">active</Badge>
                        ) : (
                          <Badge variant="warning">disabled</Badge>
                        )}
                      </td>
                      <td className="px-4 py-2.5 text-xs text-muted-foreground">
                        {formatDate(u.lastSignedInAt)}
                      </td>
                      <td className="px-4 py-2.5">
                        <div className="flex items-center justify-end gap-1">
                          {!isAdmin ? (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => updateUser(u.id, { role: 'super_admin' })}
                              disabled={busy === u.id}
                              title="Promote to super admin"
                            >
                              <ShieldCheck className="h-3.5 w-3.5" /> Promote
                            </Button>
                          ) : !isYou ? (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => updateUser(u.id, { role: 'user' })}
                              disabled={busy === u.id}
                              title="Demote to user"
                            >
                              <ShieldOff className="h-3.5 w-3.5" /> Demote
                            </Button>
                          ) : null}
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => updateUser(u.id, { isActive: !u.isActive })}
                            disabled={busy === u.id || isYou}
                            title={u.isActive ? 'Disable account' : 'Enable account'}
                          >
                            {u.isActive ? (
                              <>
                                <UserX className="h-3.5 w-3.5" /> Disable
                              </>
                            ) : (
                              <>
                                <UserCheck className="h-3.5 w-3.5" /> Enable
                              </>
                            )}
                          </Button>
                          <Button
                            size="sm"
                            variant="destructive"
                            onClick={() => deleteUser(u.id, u.email)}
                            disabled={busy === u.id || isYou}
                            title="Delete account"
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </Button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>
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
