"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";
import { LogOut, Bot, LayoutDashboard, CreditCard, Bell, ScrollText, ShieldCheck, HelpCircle, Activity, Box, Users, Settings, FileText, ArrowLeftRight } from "lucide-react";

import type { AdminShellSnapshot } from "../../lib/api/mock-data";
import { pickText, type UiLanguage, type UiTheme } from "../../lib/ui/preferences";
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

  return (
    <div className="flex h-screen flex-col bg-[#0a0e17] text-slate-200">
      {/* Top Navbar */}
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-indigo-900/50 bg-[#111827] px-4 shadow-sm z-20">
        <div className="flex items-center gap-4">
          <Link className="flex items-center gap-2 text-lg font-black tracking-tight text-white hover:text-indigo-400 transition-colors" href={withLocale(locale, "/admin/dashboard")}>
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-indigo-500 text-white shadow-sm">
              <ShieldCheck className="h-4 w-4" />
            </div>
            {snapshot.brand}
          </Link>
          <div className="hidden md:flex items-center gap-2 px-2 py-1 rounded bg-indigo-500/10 text-xs font-medium text-indigo-400 border border-indigo-500/20">
            {pickText(lang, "管理终端", "Admin Console")}
          </div>
        </div>
        <div className="flex items-center gap-4">
          <div className="hidden lg:flex items-center gap-3">
            {snapshot.quickStats.map((item) => (
              <div key={item.label} className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[#1f2937] border border-slate-700 text-xs">
                <span className="text-slate-400">{item.label}</span>
                <strong className="text-white font-bold">{item.value}</strong>
              </div>
            ))}
          </div>
          <ShellPreferences lang={lang} theme={theme} />
          <div className="h-6 w-px bg-slate-700 mx-1"></div>
          <div className="flex items-center gap-3">
            <div className="flex flex-col items-end">
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

      <div className="flex flex-1 overflow-hidden">
        {/* Minimalist Sidebar */}
        <aside className="w-16 sm:w-20 shrink-0 flex-col items-center border-r border-slate-800 bg-[#111827] py-4 flex z-10">
          <nav className="flex w-full flex-col gap-3 px-2">
            {snapshot.nav.map((item) => {
              const localizedHref = withLocale(locale, item.href);
              const isActive = isNavHrefActive(pathname, locale, item.href);
              return (
                <Link
                  className={`group relative flex h-12 w-full flex-col items-center justify-center rounded-xl transition-all ${
                    isActive ? "bg-indigo-500/10 text-indigo-400 shadow-[inset_0_0_0_1px_rgba(99,102,241,0.2)]" : "text-slate-400 hover:bg-[#1f2937] hover:text-white"
                  }`}
                  href={localizedHref}
                  key={item.href}
                  title={item.label}
                >
                  {getNavIcon(item.href)}
                  <span className="mt-1 text-[9px] font-medium opacity-0 group-hover:opacity-100 transition-opacity hidden sm:block whitespace-nowrap truncate max-w-full px-1">{item.label}</span>
                  {item.badge ? <span className="absolute top-1 right-2 h-2 w-2 rounded-full bg-amber-500 ring-2 ring-[#111827]"></span> : null}
                </Link>
              );
            })}
          </nav>
        </aside>

        {/* Main Content */}
        <main className="flex-1 overflow-y-auto p-4 md:p-6 lg:p-8">
          <div className="mx-auto flex h-full max-w-[1600px] flex-col gap-6">
            <header className="flex flex-col gap-1">
              <h1 className="text-2xl font-bold tracking-tight text-white">{snapshot.title}</h1>
              <p className="text-sm text-slate-400">{snapshot.description}</p>
            </header>

            <div className="flex flex-col gap-3">
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

            <div className="flex-1">
              {children}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
