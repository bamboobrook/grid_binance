"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useState, type ReactNode } from "react";
import {
  Activity,
  Bell,
  Bot,
  CreditCard,
  FlaskConical,
  HelpCircle,
  LayoutDashboard,
  LogOut,
  Menu,
  PanelLeftClose,
  ScrollText,
  ShieldCheck,
  WalletCards,
} from "lucide-react";

import type { NavItem, UserShellSnapshot } from "../../lib/api/mock-data";
import { pickText, type UiLanguage, type UiTheme } from "../../lib/ui/preferences";
import { MobileBottomNav } from "../layout/mobile-bottom-nav";
import { DialogFrame } from "../ui/dialog";
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
  if (href.includes("dashboard")) return <LayoutDashboard className="h-4 w-4" />;
  if (href.includes("exchange")) return <WalletCards className="h-4 w-4" />;
  if (href.includes("backtest")) return <FlaskConical className="h-4 w-4" />;
  if (href.includes("strategies")) return <Bot className="h-4 w-4" />;
  if (href.includes("billing")) return <CreditCard className="h-4 w-4" />;
  if (href.includes("notifications") || href.includes("telegram")) return <Bell className="h-4 w-4" />;
  if (href.includes("orders")) return <ScrollText className="h-4 w-4" />;
  if (href.includes("security")) return <ShieldCheck className="h-4 w-4" />;
  if (href.includes("help")) return <HelpCircle className="h-4 w-4" />;
  return <Activity className="h-4 w-4" />;
}

function navGroups(lang: UiLanguage, nav: NavItem[]) {
  const find = (part: string) => nav.find((item) => item.href.includes(part));
  return [
    {
      label: pickText(lang, "概览", "Overview"),
      items: [find("dashboard"), find("exchange"), find("strategies")].filter(Boolean) as NavItem[],
    },
    {
      label: pickText(lang, "测试", "Test"),
      items: [find("backtest")].filter(Boolean) as NavItem[],
    },
    {
      label: pickText(lang, "查询", "Query"),
      items: [find("orders"), find("analytics"), find("notifications")].filter(Boolean) as NavItem[],
    },
    {
      label: pickText(lang, "账户设置", "Account"),
      items: [find("billing"), find("security"), find("help")].filter(Boolean) as NavItem[],
    },
  ];
}

function currentNavItem(pathname: string, locale: string, nav: NavItem[]) {
  return nav.find((item) => isNavHrefActive(pathname, locale, item.href));
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
  const [isExpanded, setIsExpanded] = useState(true);
  const activeItem = currentNavItem(pathname, locale, snapshot.nav);
  const groupedNav = navGroups(lang, snapshot.nav);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="z-20 flex h-14 shrink-0 items-center justify-between border-b border-border bg-card/95 px-3 shadow-sm backdrop-blur sm:px-4">
        <div className="flex min-w-0 items-center gap-3">
          <button
            className="hidden h-9 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground sm:flex"
            onClick={() => setIsExpanded(!isExpanded)}
            title={pickText(lang, "展开/收起侧边栏", "Toggle sidebar")}
          >
            {isExpanded ? <PanelLeftClose className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
          </button>
          <Link className="flex shrink-0 items-center gap-2 font-black tracking-tight text-foreground transition-colors hover:text-primary" href={withLocale(locale, "/app/dashboard")}>
            <span className="flex h-8 w-8 items-center justify-center rounded-md bg-primary text-primary-foreground shadow-sm">
              <Bot className="h-4 w-4" />
            </span>
            <span className="hidden sm:inline">{snapshot.brand}</span>
          </Link>
          <div className="hidden min-w-0 items-center gap-2 text-xs font-semibold text-muted-foreground md:flex">
            <span>{pickText(lang, "当前位置", "Current")}</span>
            <span className="rounded-md bg-secondary px-2 py-1 text-foreground">{activeItem?.label ?? pickText(lang, "总览", "Dashboard")}</span>
          </div>
        </div>
        <div className="flex min-w-0 items-center gap-2 sm:gap-3">
          <div className="hidden items-center gap-2 xl:flex">
            <Link className="inline-flex h-9 items-center justify-center rounded-md border border-border px-3 text-xs font-bold text-foreground hover:bg-secondary" href={withLocale(locale, "/app/strategies/new")}>
              {pickText(lang, "创建机器人", "Create bot")}
            </Link>
            <Link className="inline-flex h-9 items-center justify-center rounded-md border border-border px-3 text-xs font-bold text-foreground hover:bg-secondary" href={withLocale(locale, "/app/orders")}>
              {pickText(lang, "看订单", "Orders")}
            </Link>
          </div>
          <ShellPreferences lang={lang} theme={theme} />
          <div className="hidden h-6 w-px bg-border sm:block" />
          <div className="hidden flex-col items-end sm:flex">
            <span className="text-xs font-bold text-foreground">{snapshot.identity.name}</span>
            <span className="text-[10px] text-muted-foreground">{snapshot.identity.role}</span>
          </div>
          <form action={`/api/auth/logout?locale=${locale}`} method="post">
            <button
              className="flex h-9 w-9 items-center justify-center rounded-md bg-red-500/10 text-red-500 transition-colors hover:bg-red-500/20"
              title={pickText(lang, "退出登录", "Log out")}
              type="submit"
            >
              <LogOut className="h-4 w-4" />
            </button>
          </form>
        </div>
      </header>

      <div className="relative flex flex-1 overflow-hidden pb-16 md:pb-0">
        <aside className={`hidden shrink-0 flex-col border-r border-border bg-card/95 py-4 transition-all duration-300 md:flex ${isExpanded ? "w-56 px-3" : "w-16 px-2"}`}>
          <nav className="flex w-full flex-col gap-5">
            {groupedNav.map((group) => (
              <div className="flex flex-col gap-1" key={group.label}>
                {isExpanded ? <p className="px-3 text-[11px] font-bold uppercase text-muted-foreground">{group.label}</p> : null}
                {group.items.map((item) => {
                  const localizedHref = withLocale(locale, item.href);
                  const isActive = isNavHrefActive(pathname, locale, item.href);
                  return (
                    <Link
                      className={`group relative flex h-10 w-full items-center rounded-md transition-colors ${
                        isExpanded ? "justify-start gap-3 px-3" : "justify-center px-0"
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
                      {isExpanded ? <span className="truncate text-sm font-semibold">{item.label}</span> : null}
                      {item.badge ? <span className="absolute right-2 top-2 h-2 w-2 rounded-full bg-amber-500 ring-2 ring-background" /> : null}
                    </Link>
                  );
                })}
              </div>
            ))}
          </nav>
        </aside>

        <MobileBottomNav />

        <main className="w-full flex-1 overflow-y-auto bg-background p-3 sm:p-4 md:p-5 lg:p-6">
          <div className="mx-auto flex w-full max-w-[1500px] flex-col gap-4 sm:gap-5">
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
                  lang={lang}
                  title={banner.title}
                  tone={banner.tone === "danger" ? "error" : banner.tone}
                />
              ))}
            </div>

            <div className="min-w-0 flex-1">{children}</div>
          </div>
        </main>
      </div>
    </div>
  );
}
