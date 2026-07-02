import * as React from 'react';
import { cn } from './cn';

/**
 * Inline shimmer block — drop in where data is loading.
 *   <Skeleton className="h-4 w-24" />
 */
export function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('animate-pulse rounded-md bg-accent/60', className)} {...props} />;
}
