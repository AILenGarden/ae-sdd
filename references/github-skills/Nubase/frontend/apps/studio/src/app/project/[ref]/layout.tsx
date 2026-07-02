import ProjectLayoutShell from './layout-shell';

// Static export can't enumerate runtime project refs, so we emit a single `__shell__`
// page per project route. The Java backend serves that shell for any concrete ref and
// the client reads the real value from the URL (useParams). No-op for dev/standalone.
export function generateStaticParams() {
  return [{ ref: '__shell__' }];
}

export default function ProjectLayout({
  children,
  params,
}: {
  children: React.ReactNode;
  params: { ref: string };
}) {
  return <ProjectLayoutShell projectRef={params.ref}>{children}</ProjectLayoutShell>;
}
