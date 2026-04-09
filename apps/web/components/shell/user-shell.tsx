"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import type { UserShellSnapshot } from "../../lib/api/mock-data";
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

export function UserShell({
  children,
  snapshot,
  lang,
  locale,
  theme,
}: {
  children: ReactNode;
  snapshot: UserShellSnapshot;
  lang: UiLanguage;
  locale: string;
  theme: UiTheme | null;
}) {
  const pathname = usePathname();
  const consoleStatus = [
    { label: pickText(lang, "工作区", "Workspace"), value: pickText(lang, "用户", "User") },
    { label: pickText(lang, "语言", "Language"), value: describeLanguage(lang) },
    { label: pickText(lang, "主题", "Theme"), value: describeTheme(lang, theme) },
  ];

  return (
    <div className="shell shell--workspace">
      <aside className="shell-sidebar">
        <div className="shell-sidebar__brand">
          <div className="shell-sidebar__meta">
            <Chip>{pickText(lang, "用户终端", "User console")}</Chip>
            <span>{snapshot.subtitle}</span>
          </div>
          <Link className="brand-mark" href={withLocale(locale, "/app/dashboard")}>
            {snapshot.brand}
          </Link>
          <p>{snapshot.subtitle}</p>
        </div>
        <div className="shell-sidebar__section">
          <p className="shell-sidebar__label">{pickText(lang, "导航", "Navigation")}</p>
          <nav aria-label={pickText(lang, "用户导航", "User workspace")} className="shell-sidebar__nav">
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
            <p className="shell-topbar__eyebrow">{pickText(lang, "用户控制台", "User workspace")}</p>
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
