"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import type { PublicShellSnapshot } from "../../lib/api/mock-data";
import { pickText, type UiLanguage, type UiTheme } from "../../lib/ui/preferences";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../ui/card";
import { Chip } from "../ui/chip";
import { ShellPreferences } from "./shell-preferences";

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

export function PublicShell({
  children,
  snapshot,
  lang,
  locale,
  theme,
}: {
  children: ReactNode;
  snapshot: PublicShellSnapshot;
  lang: UiLanguage;
  locale: string;
  theme: UiTheme | null;
}) {
  const pathname = usePathname();
  const marketStrip = [
    { label: pickText(lang, "终端", "Console"), value: pickText(lang, "公开", "Public") },
    { label: pickText(lang, "导航", "Routes"), value: String(snapshot.actions.length) },
    { label: pickText(lang, "支持", "Support"), value: String(snapshot.supportLinks.length) },
    { label: pickText(lang, "主题", "Theme"), value: describeTheme(lang, theme) },
  ];

  return (
    <div className="shell shell--public">
      <header className="shell-topbar shell-topbar--public">
        <div className="shell-topbar__copy shell-topbar__console">
          <div className="shell-topbar__meta">
            <Chip>{pickText(lang, "公共终端", "Public console")}</Chip>
            <Chip>{pickText(lang, "只读视图", "Read-only view")}</Chip>
          </div>
          <Link className="brand-mark" href={withLocale(locale, "/")}>
            {snapshot.brand}
          </Link>
          <p className="shell-topbar__subtitle">{snapshot.subtitle}</p>
        </div>
        <div className="shell-topbar__actions shell-topbar__actions--public">
          <nav aria-label={pickText(lang, "公共导航", "Public navigation")} className="shell-inline-nav">
            {snapshot.actions.map((item) => {
              const localizedHref = withLocale(locale, item.href);
              const isActive = isNavHrefActive(pathname, locale, item.href);
              return (
                <Link className={isActive ? "shell-link shell-link--active" : "shell-link"} href={localizedHref} key={item.href}>
                  <span>{item.label}</span>
                </Link>
              );
            })}
          </nav>
          <ShellPreferences lang={lang} theme={theme} />
        </div>
      </header>
      <div aria-label={pickText(lang, "控制台概览", "Console overview")} className="market-strip">
        {marketStrip.map((item) => (
          <div className="market-strip__item" key={item.label}>
            <span className="market-strip__label">{item.label}</span>
            <strong className="market-strip__value">{item.value}</strong>
          </div>
        ))}
      </div>
      <div className="public-shell__layout">
        <aside className="public-shell__aside">
          <div className="hero-block">
            <p className="hero-block__eyebrow">{snapshot.eyebrow}</p>
            <h1>{snapshot.title}</h1>
            <p>{snapshot.description}</p>
          </div>
          <div className="stack-grid">
            {snapshot.highlights.map((item) => (
              <Card className="public-shell__highlight" key={item.title}>
                <CardHeader>
                  <CardTitle>{item.title}</CardTitle>
                  <CardDescription>{item.description}</CardDescription>
                </CardHeader>
              </Card>
            ))}
          </div>
        </aside>
        <div className="public-shell__content">{children}</div>
        <aside className="public-shell__rail">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "运行基线", "Operational baseline")}</CardTitle>
              <CardDescription>
                {pickText(lang, "V1 保持计费与交易风险表面明确。", "V1 keeps billing and trading risk surfaces explicit.")}
              </CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {snapshot.supportLinks.map((item) => (
                  <li key={item.href}>
                    <Link href={withLocale(locale, item.href)}>{item.label}</Link>
                  </li>
                ))}
              </ul>
            </CardBody>
          </Card>
        </aside>
      </div>
    </div>
  );
}
