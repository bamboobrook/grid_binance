"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useState, type ReactNode } from "react";
import {
  Activity,
  ArrowLeftRight,
  Bot,
  Box,
  CreditCard,
  FileText,
  LayoutDashboard,
  LogOut,
  Settings,
  ShieldCheck,
  Users,
} from "lucide-react";

import type { AdminShellSnapshot } from "../../lib/api/mock-data";
import { pickText, type UiLanguage, type UiTheme } from "../../lib/ui/preferences";
import { StatusBanner } from "../ui/status-banner";
import { ShellPreferences } from "./shell-preferences";

function withLocale(locale: string, href: string) {
  if (!href.startsWith("/")) return href;
  if (href === "/") return `/${locale}`;
  return `/${locale}${href}`;
}

function isNavHrefActive(pathname: string, locale: string, href: string) {
  const localized = withLocale(locale, href);
  return pathname === localized || pathname.startsWith(`${localized}/`);
}

function getNavIcon(href: string) {
  if (href.includes("dashboard")) return <LayoutDashboard className="h-5 w-5" />;
  if (href.includes("strategies")) return <Bot className="h-5 w-5" />;
  if (href.includes("billing") || href.includes("memberships")) return <CreditCard className="h-5 w-5" />;
  if (href.includes("deposits") || href.includes("sweeps")) return <ArrowLeftRight className="h-5 w-5" />;
  if (href.includes("users") || href.includes("address-pools")) return <Users className="h-5 w-5" />;
  if (href.includes("system") || href.includes("audit")) return <Settings className="h-5 w-5" />;
  if (href.includes("templates")) return <FileText className="h-5 w-5" />;
  if (href.includes("analytics")) return <Activity className="h-5 w-5" />;
  return <Box className="h-5 w-5" />;
}

export function AdminShell({
  children,
  snapshot,
  lang,
  locale,
  theme,
}: {
  children: ReactNode;
  snapshot: AdminShellSnapshot;
  lang: UiLanguage;
  locale: string;
  theme: UiTheme | null;
}) {
  const pathname = usePathname();
  const [isExpanded, setIsExpanded] = useState(false);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="z-20 flex h-14 shrink-0 items-center justify-between border-b border-border bg-card/95 px-4 shadow-sm backdrop-blur">
        <div className="flex items-center gap-4">
          <Link
            className="flex items-center gap-2 text-lg font-black tracking-tight text-foreground transition-colors hover:text-primary"
            href={withLocale(locale, "/admin/dashboard")}
          >
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
              <ShieldCheck className="h-4 w-4" />
            </div>
            {snapshot.brand}
          </Link>
          <div className="hidden items-center gap-2 rounded border border-primary/20 bg-primary/10 px-2 py-1 text-xs font-medium text-primary md:flex">
            {pickText(lang, "管理终端", "Admin Console")}
          </div>
        </div>
        <div className="flex items-center gap-4">
          <div className="hidden items-center gap-3 lg:flex">
            {snapshot.quickStats.map((item) => (
              <div key={item.label} className="flex items-center gap-2 rounded-lg border border-border bg-secondary px-3 py-1.5 text-xs">
                <span className="text-muted-foreground">{item.label}</span>
                <strong className="font-bold text-foreground">{item.value}</strong>
              </div>
            ))}
          </div>
          <ShellPreferences lang={lang} theme={theme} />
          <div className="mx-1 h-6 w-px bg-border" />
          <div className="flex items-center gap-3">
            <div className="flex flex-col items-end">
              <span className="text-xs font-bold text-foreground">{snapshot.identity.name}</span>
              <span className="text-[10px] text-muted-foreground">{snapshot.identity.role}</span>
            </div>
            <form action={`/api/auth/logout?locale=${locale}`} method="post">
              <button
                type="submit"
                className="flex h-8 w-8 items-center justify-center rounded-lg bg-red-500/10 text-red-500 transition-colors hover:bg-red-500/20"
                title={pickText(lang, "退出登录", "Log Out")}
              >
                <LogOut className="h-4 w-4" />
              </button>
            </form>
          </div>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden relative pb-16 sm:pb-0">
        <aside className={`fixed bottom-0 left-0 right-0 sm:relative z-20 sm:z-10 flex flex-row sm:flex-col shrink-0 items-center sm:items-start justify-around sm:justify-start border-t sm:border-t-0 sm:border-r border-border bg-card/95 py-2 sm:py-4 transition-all duration-300 ${isExpanded ? "sm:w-48 md:w-56 px-2 sm:px-4" : "sm:w-16 md:w-20 px-2 sm:px-2 items-center"}`}>
          <nav className="flex flex-row sm:flex-col w-full gap-1 sm:gap-3 justify-around sm:justify-start">
            {snapshot.nav.map((item) => {
              const localizedHref = withLocale(locale, item.href);
              const isActive = isNavHrefActive(pathname, locale, item.href);
              return (
                <Link
                  className={`group relative flex h-12 w-12 sm:w-full transition-all rounded-xl ${
                    isExpanded ? "flex-row items-center justify-start px-4 gap-3" : "flex-col items-center justify-center"
                  } ${
                    isActive
                      ? "bg-primary/10 text-primary ring-1 ring-primary/20"
                      : "text-muted-foreground hover:bg-secondary hover:text-foreground"
                  }`}
                  href={localizedHref}
                  key={item.href}
                  title={item.label}
                >
                  {getNavIcon(item.href)}
                  <span className={`${isExpanded ? "mt-0 text-sm font-semibold opacity-100 block" : "mt-1 text-[9px] font-medium opacity-0 group-hover:opacity-100 hidden md:block"} transition-all whitespace-nowrap truncate max-w-full px-1`}>{item.label}</span>
                  {item.badge ? <span className={`absolute rounded-full bg-amber-500 ring-2 ring-background ${isExpanded ? "top-1/2 -translate-y-1/2 right-3 h-2 w-2" : "top-1 right-2 h-2 w-2"}`} /> : null}
                </Link>
              );
            })}
          </nav>
        </aside>

        <main className="flex-1 overflow-y-auto p-3 sm:p-4 md:p-6 lg:p-8 w-full">
          <div className="mx-auto flex h-full max-w-[1600px] flex-col gap-4 sm:gap-6">
            <header className="flex flex-col gap-1">
              <h1 className="text-2xl font-bold tracking-tight text-foreground">{snapshot.title}</h1>
              <p className="text-sm text-muted-foreground">{snapshot.description}</p>
            </header>

            <div className="flex flex-col gap-3">
              {snapshot.banners.map((banner) => (
                <StatusBanner
                        tone="info"
                        lang={lang}
                  action={banner.action ? { ...banner.action, href: withLocale(locale, banner.action.href) } : undefined}
                  description={banner.description}
                  key={banner.title}
                  title={banner.title}
                />
              ))}
            </div>

            <div className="flex-1">{children}</div>
          </div>
        </main>
      </div>
    </div>
  );
}
