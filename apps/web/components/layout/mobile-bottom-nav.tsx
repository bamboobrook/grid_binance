"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useLocale } from "next-intl";
import { Activity, Bot, History, LayoutDashboard, ShieldCheck } from "lucide-react";
import { cn } from "@/lib/utils";

export function MobileBottomNav() {
  const locale = useLocale();
  const pathname = usePathname();
  const zh = locale === "zh";

  const items = [
    { name: zh ? "总览" : "Home", href: "/app/dashboard", icon: LayoutDashboard },
    { name: zh ? "机器人" : "Bots", href: "/app/strategies", icon: Bot },
    { name: zh ? "记录" : "Orders", href: "/app/orders", icon: History },
    { name: zh ? "统计" : "Stats", href: "/app/analytics", icon: Activity },
    { name: zh ? "账户" : "Account", href: "/app/security", icon: ShieldCheck },
  ];

  const isCurrent = (href: string) =>
    pathname === `/${locale}${href}` ||
    pathname.startsWith(`/${locale}${href}/`);

  return (
    <nav className="fixed inset-x-0 bottom-0 z-50 flex items-center justify-around border-t border-border bg-card/95 backdrop-blur-sm pb-safe md:hidden">
      {items.map((item) => {
        const active = isCurrent(item.href);
        return (
          <Link
            key={item.href}
            href={`/${locale}${item.href}`}
            className={cn(
              "flex min-h-[48px] min-w-[48px] flex-col items-center justify-center gap-0.5 px-2 py-1 text-[10px] font-medium transition-colors",
              active
                ? "text-primary"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <item.icon
              className={cn("h-5 w-5", active ? "text-primary" : "text-muted-foreground")}
            />
            {item.name}
          </Link>
        );
      })}
    </nav>
  );
}
