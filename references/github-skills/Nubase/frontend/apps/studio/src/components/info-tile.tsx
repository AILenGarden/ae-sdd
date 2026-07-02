export function InfoTile({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-lg border border-border bg-background p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className={'mt-2 truncate text-sm ' + (mono ? 'font-mono' : 'font-medium')}>{value}</div>
    </div>
  );
}
