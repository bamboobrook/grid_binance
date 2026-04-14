"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";
import { LogOut, Bot, LayoutDashboard, CreditCard, Bell, ScrollText, ShieldCheck, HelpCircle, Activity, Box } from "lucide-react";

import type { UserShellSnapshot } from "../../lib/api/mock-data";
import { pickText, type UiLanguage, type UiTheme } from "../../lib/ui/preferences";
import { DialogFrame } from "../ui/dialog";
import { StatusBanner } from "../ui/status-banner";
import { ShellPreferences } from "./shell-preferences";

function describeLanguage(lang: UiLanguage) {
  return pickText(lang, "中文", "English");
}

function describeTheme(lang: UiLanguage, theme: UiTheme | null) {
  if (theme === "dark") return pickText(lang, "深色", "Dark");
  if (theme === "light") return pickText(lang, "浅色", "Light");
  return pickText(lang, "跟随系统", "System");
}

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
  if (href.includes("billing")) return <CreditCard className="h-5 w-5" />;
  if (href.includes("notifications")) return <Bell className="h-5 w-5" />;
  if (href.includes("orders")) return <ScrollText className="h-5 w-5" />;
  if (href.includes("security")) return <ShieldCheck className="h-5 w-5" />;
  if (href.includes("help") || href.includes("telegram")) return <HelpCircle className="h-5 w-5" />;
  if (href.includes("analytics")) return <Activity className="h-5 w-5" />;
  return <Box className="h-5 w-5" />;
}

export function UserShell({
  children,
  snapshot,
  lang,
  locale,
  theme,
  expiryReminder,
}: {
  children: ReactNode;
  snapshot: UserShellSnapshot;
  lang: UiLanguage;
  locale: string;
  theme: UiTheme | null;
  expiryReminder?: { description: string; title: string } | null;
}) {
  const pathname = usePathname();

  return (
    <div className="flex h-screen flex-col bg-[#0f141f] text-slate-200">
      {/* Top Navbar */}
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-slate-800 bg-[#111827] px-4 shadow-sm z-20">
        <div className="flex items-center gap-4">
          <Link className="flex items-center gap-2 text-lg font-black tracking-tight text-white hover:text-primary transition-colors" href={withLocale(locale, "/app/dashboard")}>
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
              <Bot className="h-4 w-4" />
            </div>
            <span className="hidden sm:inline">{snapshot.brand}</span>
          </Link>
          <div className="hidden md:flex items-center gap-2 px-2 py-1 rounded bg-[#1f2937] text-xs font-medium text-slate-300 border border-slate-700">
            {pickText(lang, "用户工作区", "User Workspace")}
          </div>
        </div>
        <div className="flex items-center gap-2 sm:gap-4">
          <div className="hidden lg:flex items-center gap-3">
            {snapshot.quickStats.map((item) => (
              <div key={item.label} className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[#1f2937] border border-slate-700 text-xs">
                <span className="text-slate-400">{item.label}</span>
                <strong className="text-white font-bold">{item.value}</strong>
              </div>
            ))}
          </div>
          <ShellPreferences lang={lang} theme={theme} />
          <div className="h-6 w-px bg-slate-700 mx-0 sm:mx-1"></div>
          <div className="flex items-center gap-2 sm:gap-3">
            <div className="hidden sm:flex flex-col items-end">
              <span className="text-xs font-bold text-white">{snapshot.identity.name}</span>
              <span className="text-[10px] text-slate-400">{snapshot.identity.role}</span>
            </div>
            <form action={`/api/auth/logout?locale=${locale}`} method="post">
              <button type="submit" className="flex h-8 w-8 items-center justify-center rounded-lg bg-red-500/10 text-red-500 hover:bg-red-500/20 transition-colors" title={pickText(lang, "退出登录", "Log Out")}>
                <LogOut className="h-4 w-4" />
              </button>
            </form>
          </div>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden relative pb-16 sm:pb-0">
        {/* Minimalist Sidebar (Bottom Nav on Mobile) */}
        <aside className="fixed bottom-0 left-0 right-0 sm:relative sm:w-16 md:w-20 shrink-0 flex-row sm:flex-col items-center sm:border-r border-t sm:border-t-0 border-slate-800 bg-[#111827] py-2 sm:py-4 flex z-20 sm:z-10 justify-around sm:justify-start">
          <nav className="flex flex-row sm:flex-col w-full gap-1 sm:gap-3 px-2 sm:px-2 justify-around sm:justify-start">
            {snapshot.nav.map((item) => {
              const localizedHref = withLocale(locale, item.href);
              const isActive = isNavHrefActive(pathname, locale, item.href);
              return (
                <Link
                  className={`group relative flex h-12 w-12 sm:w-full flex-col items-center justify-center rounded-xl transition-all ${
                    isActive ? "bg-primary/10 text-primary shadow-[inset_0_0_0_1px_rgba(59,130,246,0.2)]" : "text-slate-400 hover:bg-[#1f2937] hover:text-white"
                  }`}
                  href={localizedHref}
                  key={item.href}
                  title={item.label}
                >
                  {getNavIcon(item.href)}
                  <span className="mt-1 text-[9px] font-medium opacity-0 group-hover:opacity-100 transition-opacity hidden md:block whitespace-nowrap truncate max-w-full px-1">{item.label}</span>
                  {item.badge ? <span className="absolute top-1 right-2 h-2 w-2 rounded-full bg-amber-500 ring-2 ring-[#111827]"></span> : null}
                </Link>
              );
            })}
          </nav>
        </aside>

        {/* Main Content */}
        <main className="flex-1 overflow-y-auto bg-[#0a0e17] p-3 sm:p-4 md:p-6 lg:p-8 w-full">
          <div className="mx-auto flex h-full w-full max-w-[1600px] flex-col gap-4 sm:gap-6">
            <header className="flex flex-col gap-1">
              <h1 className="text-xl sm:text-2xl font-bold tracking-tight text-white">{snapshot.title}</h1>
              <p className="text-xs sm:text-sm text-slate-400">{snapshot.description}</p>
            </header>

            <div className="flex flex-col gap-3">
              {expiryReminder ? (
                <DialogFrame description={expiryReminder.description} lang={lang} modal title={expiryReminder.title} tone="warning">
                  <Link className="button button--ghost" href={withLocale(locale, "/app/billing")}>
                    {pickText(lang, "打开会员中心", "Open membership center")}
                  </Link>
                </DialogFrame>
              ) : null}
              {snapshot.banners.map((banner) => (
                <StatusBanner
                  action={banner.action ? { ...banner.action, href: withLocale(locale, banner.action.href) } : undefined}
                  description={banner.description}
                  key={banner.title}
                  title={banner.title}
                  tone={banner.tone}
                />
              ))}
            </div>

            <div className="flex-1 w-full">
              {children}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
