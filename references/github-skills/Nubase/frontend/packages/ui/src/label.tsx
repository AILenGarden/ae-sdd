import * as React from 'react';
import { cn } from './cn';

export const Label = React.forwardRef<HTMLLabelElement, React.LabelHTMLAttributes<HTMLLabelElement>>(
  ({ className, ...props }, ref) => (
    <label
      ref={ref}
      className={cn('text-sm font-medium leading-none text-foreground', className)}
      {...props}
    />
  )
);
Label.displayName = 'Label';
