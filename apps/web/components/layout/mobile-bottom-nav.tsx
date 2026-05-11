"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useLocale, useTranslations } from "next-intl";
import { LayoutDashboard, ArrowLeftRight, History, User } from "lucide-react";
import { cn } from "@/lib/utils";

export function MobileBottomNav() {
  const t = useTranslations("common.sidebar");
  const locale = useLocale();
  const pathname = usePathname();

  const items = [
    { name: t("dashboard"), href: "/app/dashboard", icon: LayoutDashboard },
    { name: t("strategies"), href: "/app/strategies", icon: ArrowLeftRight },
    { name: t("orders"), href: "/app/orders", icon: History },
    { name: t("settings"), href: "/app/security", icon: User },
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
