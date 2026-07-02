// Server layout solely to provide generateStaticParams for the [id] segment under
// static export (the page itself is a client component and can't export it). Emits one
// `__shell__` page; the Java backend serves it for any concrete memory id.
export function generateStaticParams() {
  return [{ id: '__shell__' }];
}

export default function MemoryIdLayout({ children }: { children: React.ReactNode }) {
  return children;
}
