'use client';

import * as React from 'react';
import { CheckCircle2, AlertTriangle, XCircle, Info, X } from 'lucide-react';
import { cn } from './cn';

export type ToastVariant = 'success' | 'error' | 'warning' | 'info';

interface ToastItem {
  id: number;
  title?: string;
  message: string;
  variant: ToastVariant;
}

interface ToastContextValue {
  toast: (input: { title?: string; message: string; variant?: ToastVariant }) => void;
}

const ToastContext = React.createContext<ToastContextValue | null>(null);

/**
 * Wrap the tree once near the root. `useToast` works anywhere below it.
 * Toasts auto-dismiss after 4 seconds; user can click × to dismiss earlier.
 */
export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = React.useState<ToastItem[]>([]);

  const dismiss = React.useCallback((id: number) => {
    setToasts((ts) => ts.filter((t) => t.id !== id));
  }, []);

  const toast = React.useCallback<ToastContextValue['toast']>((input) => {
    const id = Date.now() + Math.random();
    setToasts((ts) => [...ts, { id, variant: input.variant ?? 'info', title: input.title, message: input.message }]);
    setTimeout(() => dismiss(id), 4000);
  }, [dismiss]);

  return (
    <ToastContext.Provider value={{ toast }}>
      {children}
      <div className="pointer-events-none fixed inset-x-0 bottom-4 z-50 flex flex-col items-center gap-2 px-4">
        {toasts.map((t) => (
          <ToastView key={t.id} item={t} onClose={() => dismiss(t.id)} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastContextValue {
  const ctx = React.useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used inside <ToastProvider>');
  return ctx;
}

function ToastView({ item, onClose }: { item: ToastItem; onClose: () => void }) {
  const Icon =
    item.variant === 'success'
      ? CheckCircle2
      : item.variant === 'warning'
        ? AlertTriangle
        : item.variant === 'error'
          ? XCircle
          : Info;
  const tone = {
    success: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-200',
    warning: 'border-amber-500/30 bg-amber-500/10 text-amber-200',
    error: 'border-destructive/40 bg-destructive/10 text-red-200',
    info: 'border-border bg-card text-foreground',
  }[item.variant];
  return (
    <div
      className={cn(
        'pointer-events-auto flex w-full max-w-sm items-start gap-3 rounded-lg border px-4 py-3 shadow-lg backdrop-blur',
        tone
      )}
      role="status"
    >
      <Icon className="mt-0.5 h-4 w-4 shrink-0" />
      <div className="flex-1 min-w-0 text-sm">
        {item.title ? <p className="font-medium">{item.title}</p> : null}
        <p className="break-words text-xs opacity-90">{item.message}</p>
      </div>
      <button onClick={onClose} className="shrink-0 rounded p-0.5 opacity-70 hover:opacity-100" aria-label="Dismiss">
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
