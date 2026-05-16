"use client";

import { clsx } from "clsx";

export function TouchButton({
  children,
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={clsx(
        "min-h-[44px] min-w-[44px] rounded-md px-4 py-2.5 text-sm font-medium",
        "transition-colors active:scale-[0.98]",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}

export function TouchInput({
  className,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={clsx(
        "h-12 rounded-md border bg-background px-4 text-sm",
        "focus:ring-2 focus:ring-primary/20 focus:outline-none",
        className,
      )}
      {...props}
    />
  );
}

export function TouchSelect({
  className,
  ...props
}: React.SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select
      className={clsx(
        "h-12 rounded-md border bg-background px-4 text-sm",
        className,
      )}
      {...props}
    />
  );
}
