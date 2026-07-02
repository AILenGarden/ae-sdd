'use client';

import { Card, CardContent } from '@nubase/ui';

/** Shared presentational form helpers for the Authentication admin pages. */

// Common field styling — mirrors @nubase/ui Input (focus ring, hover, shadow, transition) at the
// compact density used across the auth settings pages.
const FIELD =
  'rounded-md border border-input bg-transparent px-2.5 py-1 text-xs shadow-sm transition-colors ' +
  'placeholder:text-muted-foreground hover:border-muted-foreground/40 ' +
  'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring ' +
  'disabled:cursor-not-allowed disabled:opacity-50';

export function SectionCard({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <Card className="overflow-hidden transition-shadow hover:shadow-md">
      <CardContent className="p-0">
        <div className="flex items-start gap-3 border-b border-border/60 bg-muted/20 px-4 py-3">
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border/60 bg-background">
            <Icon className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="min-w-0 pt-0.5">
            <h3 className="text-sm font-semibold leading-none tracking-tight">{title}</h3>
            {description && <p className="mt-1 text-[11px] leading-snug text-muted-foreground">{description}</p>}
          </div>
        </div>
        <dl className="divide-y divide-border/40 px-4 text-xs">{children}</dl>
      </CardContent>
    </Card>
  );
}

export function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[minmax(160px,240px)_1fr] items-center gap-3 py-2">
      <dt className="leading-snug text-muted-foreground">{label}</dt>
      <dd className="flex min-w-0 items-center text-foreground">{children}</dd>
    </div>
  );
}

export function BoolInput({
  value,
  onChange,
  disabled,
}: {
  value: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className={'inline-flex select-none items-center gap-2 ' + (disabled ? 'opacity-50' : 'cursor-pointer')}>
      <input
        type="checkbox"
        checked={value}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
        className="h-4 w-4 cursor-pointer rounded border-input accent-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed"
      />
      <span className={'text-xs ' + (value ? 'font-medium text-foreground' : 'text-muted-foreground')}>
        {value ? 'on' : 'off'}
      </span>
    </label>
  );
}

export function NumberInput({
  value,
  onChange,
  min,
  max,
  step,
  disabled,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
}) {
  return (
    <input
      type="number"
      value={value}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      onChange={(e) => {
        const n = Number(e.target.value);
        if (!Number.isNaN(n)) onChange(n);
      }}
      className={'w-28 tabular-nums ' + FIELD}
    />
  );
}

export function TextInput({
  value,
  onChange,
  placeholder,
  disabled,
  type = 'text',
  className = 'w-full max-w-md',
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  disabled?: boolean;
  type?: string;
  className?: string;
}) {
  return (
    <input
      type={type}
      value={value}
      placeholder={placeholder}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value)}
      className={className + ' ' + FIELD}
    />
  );
}

export function SelectInput({
  value,
  onChange,
  options,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  options: string[];
  disabled?: boolean;
}) {
  return (
    <select
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value)}
      className={'min-w-32 cursor-pointer ' + FIELD}
    >
      {options.map((o) => (
        <option key={o} value={o}>
          {o}
        </option>
      ))}
    </select>
  );
}
