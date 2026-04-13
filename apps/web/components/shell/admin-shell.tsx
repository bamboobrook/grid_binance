"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import type { AdminShellSnapshot } from "../../lib/api/mock-data";
import { pickText, type UiLanguage, type UiTheme } from "../../lib/ui/preferences";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../ui/card";
import { Chip } from "../ui/chip";
import { StatusBanner } from "../ui/status-banner";
import { ShellPreferences } from "./shell-preferences";

function describeLanguage(lang: UiLanguage) {
  return pickText(lang, "中文", "English");
}

function describeTheme(lang: UiLanguage, theme: UiTheme | null) {
  if (theme === "dark") {
    return pickText(lang, "深色", "Dark");
  }
  if (theme === "light") {
    return pickText(lang, "浅色", "Light");
  }
  return pickText(lang, "跟随系统", "System");
}

function withLocale(locale: string, href: string) {
  if (!href.startsWith("/")) {
    return href;
  }
  if (href === "/") {
    return `/${locale}`;
  }
  return `/${locale}${href}`;
}

function isNavHrefActive(pathname: string, locale: string, href: string) {
  const localized = withLocale(locale, href);
  return pathname === localized || pathname.startsWith(`${localized}/`);
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
  const consoleStatus = [
    { label: pickText(lang, "控制面", "Console"), value: pickText(lang, "管理后台", "Admin") },
    { label: pickText(lang, "语言", "Language"), value: describeLanguage(lang) },
    { label: pickText(lang, "主题", "Theme"), value: describeTheme(lang, theme) },
  ];

  return (
    <div className="shell shell--workspace shell--admin">
      <aside className="shell-sidebar shell-sidebar--admin">
        <div className="shell-sidebar__brand">
          <div className="shell-sidebar__meta">
            <Chip>{pickText(lang, "管理终端", "Admin console")}</Chip>
            <span>{snapshot.subtitle}</span>
          </div>
          <Link className="brand-mark" href={withLocale(locale, "/admin/dashboard")}>
            {snapshot.brand}
          </Link>
          <p>{snapshot.subtitle}</p>
        </div>
        <div className="shell-sidebar__section">
          <p className="shell-sidebar__label">{pickText(lang, "导航", "Navigation")}</p>
          <nav aria-label={pickText(lang, "管理员导航", "Admin workspace")} className="shell-sidebar__nav">
            {snapshot.nav.map((item) => {
              const localizedHref = withLocale(locale, item.href);
              const isActive = isNavHrefActive(pathname, locale, item.href);
              return (
                <Link className={isActive ? "shell-link shell-link--active" : "shell-link"} href={localizedHref} key={item.href}>
                  <span>{item.label}</span>
                  {item.badge ? <Chip tone={isActive ? "warning" : "default"}>{item.badge}</Chip> : null}
                </Link>
              );
            })}
          </nav>
        </div>
        <div className="shell-sidebar__section">
          <p className="shell-sidebar__label">{pickText(lang, "会话", "Session")}</p>
          <Card className="shell-sidebar__identity">
            <CardHeader>
              <CardTitle>{snapshot.identity.name}</CardTitle>
              <CardDescription>{snapshot.identity.role}</CardDescription>
            </CardHeader>
            <CardBody>
              <p>{snapshot.identity.context}</p>
            </CardBody>
            <CardFooter className="pt-0">
              <form action={`/api/auth/logout?locale=${locale}`} method="post" className="w-full">
                <button type="submit" className="flex w-full items-center justify-center gap-2 rounded-lg bg-red-500/10 px-3 py-2 text-sm font-medium text-red-500 transition-colors hover:bg-red-500/20 hover:text-red-600">
                  <LogOut className="h-4 w-4" />
                  {pickText(lang, "退出登录", "Log Out")}
                </button>
              </form>
            </CardFooter>
          </Card>
        </div>
      </aside>
      <div className="shell-content">
        <header className="shell-topbar">
          <div className="shell-topbar__copy shell-topbar__console">
            <div className="shell-topbar__meta">
              {consoleStatus.map((item) => (
                <div className="shell-topbar__status" key={item.label}>
                  <span>{item.label}</span>
                  <strong>{item.value}</strong>
                </div>
              ))}
            </div>
            <p className="shell-topbar__eyebrow">{pickText(lang, "管理后台", "Admin operations")}</p>
            <h1>{snapshot.title}</h1>
            <p className="shell-topbar__subtitle">{snapshot.description}</p>
          </div>
          <div className="shell-topbar__actions">
            <ShellPreferences lang={lang} theme={theme} />
            <div className="metric-strip">
              {snapshot.quickStats.map((item) => (
                <div className="metric-strip__item" key={item.label}>
                  <span>{item.label}</span>
                  <strong>{item.value}</strong>
                </div>
              ))}
            </div>
          </div>
        </header>
        <div className="shell-banner-stack">
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
        <main className="shell-main">{children}</main>
      </div>
    </div>
  );
}
