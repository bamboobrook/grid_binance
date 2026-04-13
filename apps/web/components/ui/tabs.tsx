"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";

type TabItem = {
  href: string;
  label: string;
};

export function Tabs({
  activeHref,
  items,
  label,
}: {
  activeHref?: string;
  items: readonly TabItem[];
  label: string;
}) {
  const pathname = usePathname();

  return (
    <nav aria-label={label} className="ui-tabs mb-4 flex items-center gap-1 border-b border-border/60 pb-px">
      {items.map((item) => {
        const isActive = activeHref ? activeHref === item.href : pathname.includes(item.href);

        return (
          <Link
            className={cn(
              "ui-tab rounded-t-sm border-b-2 px-4 py-2 text-sm font-semibold transition-colors",
              isActive 
                ? "ui-tab__meta bg-primary/5 text-primary border-primary" 
                : "text-muted-foreground border-transparent hover:text-foreground hover:bg-secondary/30"
            )}
            href={item.href}
            key={item.href}
          >
            <span>{item.label}</span>
          </Link>
        );
      })}
    </nav>
  );
}
