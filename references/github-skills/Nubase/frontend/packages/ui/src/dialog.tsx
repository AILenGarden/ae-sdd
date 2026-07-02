'use client';

import * as React from 'react';
import { X } from 'lucide-react';
import { cn } from './cn';

interface DialogProps {
  open: boolean;
  onClose?: () => void;
  onOpenChange?: (open: boolean) => void;
  children: React.ReactNode;
  /** Tailwind width class. Defaults to `max-w-md`. */
  size?: string;
}

/**
 * Minimal modal: backdrop + centered card. Esc and backdrop click both close.
 * Renders nothing when closed so consumers can mount it unconditionally.
 */
export function Dialog({ open, onClose, onOpenChange, children, size = 'max-w-md' }: DialogProps) {
  const close = React.useCallback(() => {
    onOpenChange?.(false);
    onClose?.();
  }, [onClose, onOpenChange]);

  React.useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    document.addEventListener('keydown', onKey);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.removeEventListener('keydown', onKey);
      document.body.style.overflow = prevOverflow;
    };
  }, [open, close]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 p-4 backdrop-blur-sm"
      onClick={close}
      role="dialog"
      aria-modal="true"
    >
      <div
        className={cn(
          'w-full overflow-hidden rounded-lg border border-border bg-card text-card-foreground shadow-2xl shadow-slate-950/15',
          size
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>
  );
}

export function DialogHeader({
  children,
  title,
  description,
  onClose,
}: {
  children?: React.ReactNode;
  title?: string;
  description?: string;
  onClose?: () => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-border bg-muted/35 px-5 py-4">
      <div className="space-y-1">
        <h2 className="text-base font-semibold">{title ?? children}</h2>
        {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
      </div>
      {onClose ? (
        <button
          onClick={onClose}
          className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          aria-label="Close"
        >
          <X className="h-4 w-4" />
        </button>
      ) : null}
    </div>
  );
}

export function DialogBody({ children, className }: { children: React.ReactNode; className?: string }) {
  return <div className={cn('px-5 py-4', className)}>{children}</div>;
}

export function DialogFooter({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-end gap-2 border-t border-border bg-muted/35 px-5 py-3">
      {children}
    </div>
  );
}
