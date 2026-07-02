'use client';

import { WorkspaceShell } from '@/components/workspace-shell';
import { useProjectRef } from '@/lib/route-params';

export default function ProjectLayoutShell({
  children,
  projectRef,
}: {
  children: React.ReactNode;
  projectRef: string;
}) {
  const resolvedProjectRef = useProjectRef(projectRef);
  return <WorkspaceShell projectRef={resolvedProjectRef}>{children}</WorkspaceShell>;
}
