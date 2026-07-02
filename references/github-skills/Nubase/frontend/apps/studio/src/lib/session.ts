'use client';

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface PlatformUser {
  id: string;
  email: string;
  fullName?: string | null;
  /** 'super_admin' | 'user' */
  role?: string | null;
}

export function isSuperAdmin(user: PlatformUser | null | undefined): boolean {
  return user?.role?.toLowerCase() === 'super_admin';
}

export interface ProjectContext {
  ref: string;
  apikey: string;
  name?: string | null;
  initStatus?: string | null;
  healthStatus?: string | null;
}

interface SessionState {
  /** Platform-user JWT issued by /auth/v1/platform/token. Used as apikey for /admin/projects. */
  platformKey: string | null;
  /** Currently signed-in platform user. */
  user: PlatformUser | null;
  /** Currently-selected project; `apikey` is its service_role token. */
  project: ProjectContext | null;
  /** True once zustand has finished reading from localStorage. Components should wait on this
   *  before deciding "no session → redirect to /login", otherwise a fresh page load races the
   *  rehydration and bounces the user to login. */
  hasHydrated: boolean;
  setAuth: (auth: { platformKey: string; user: PlatformUser }) => void;
  setProject: (project: ProjectContext | null) => void;
  signOut: () => void;
  setHasHydrated: (v: boolean) => void;
}

export const useSession = create<SessionState>()(
  persist(
    (set) => ({
      platformKey: null,
      user: null,
      project: null,
      hasHydrated: false,
      setAuth: ({ platformKey, user }) => set({ platformKey, user }),
      setProject: (project) => set({ project }),
      signOut: () => set({ platformKey: null, user: null, project: null }),
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
    }),
    {
      name: 'nubase.session',
      partialize: (s) => ({ platformKey: s.platformKey, user: s.user, project: s.project }),
      onRehydrateStorage: () => (state) => {
        state?.setHasHydrated(true);
      },
    }
  )
);

/** True when the project has a real underlying database we can query. */
export function isProjectReady(p: ProjectContext | null | undefined): boolean {
  if (!p) return false;
  if (!p.initStatus) return true; // unknown — be permissive
  return p.initStatus.toUpperCase() === 'INITIALIZED';
}
