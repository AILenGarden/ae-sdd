import { WorkspaceShell } from '@/components/workspace-shell';

export default function AppLayout({ children }: { children: React.ReactNode }) {
  return <WorkspaceShell>{children}</WorkspaceShell>;
}
