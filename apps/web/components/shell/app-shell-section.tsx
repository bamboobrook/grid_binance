import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

export function AppShellSection({
  actions,
  children,
  className,
  description,
  eyebrow,
  title,
}: {
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  description?: string;
  eyebrow?: string;
  title: string;
}) {
  return (
    <section className={cn("app-section flex flex-col gap-4", className)}>
      <header className="app-section__header flex flex-col justify-between gap-4 border-b border-border/60 pb-2 sm:flex-row sm:items-end">
        <div className="space-y-1">
          {eyebrow && <p className="text-[10px] font-bold text-primary uppercase tracking-widest">{eyebrow}</p>}
          <h1 className="text-xl font-bold text-foreground tracking-tight">{title}</h1>
          {description && <p className="text-xs text-muted-foreground mt-1 max-w-2xl leading-relaxed">{description}</p>}
        </div>
        {actions && <div className="flex items-center gap-2 shrink-0">{actions}</div>}
      </header>
      <div className="app-section__content flex flex-col gap-4">{children}</div>
    </section>
  );
}
