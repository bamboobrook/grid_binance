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
    <nav aria-label={label} className="flex items-center gap-1 border-b border-slate-800/60 pb-px mb-4">
      {items.map((item) => {
        const isActive = activeHref ? activeHref === item.href : pathname.includes(item.href);

        return (
          <Link
            className={cn(
              "px-4 py-2 text-sm font-semibold rounded-t-sm transition-colors border-b-2",
              isActive 
                ? "text-primary border-primary bg-primary/5" 
                : "text-slate-500 border-transparent hover:text-slate-300 hover:bg-slate-800/30"
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
