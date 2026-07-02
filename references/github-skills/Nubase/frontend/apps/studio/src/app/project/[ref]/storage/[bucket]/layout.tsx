// Server layout solely to provide generateStaticParams for the [bucket] segment under
// static export (the page itself is a client component and can't export it). Emits one
// `__shell__` page; the Java backend serves it for any concrete bucket name.
export function generateStaticParams() {
  return [{ bucket: '__shell__' }];
}

export default function BucketLayout({ children }: { children: React.ReactNode }) {
  return children;
}
