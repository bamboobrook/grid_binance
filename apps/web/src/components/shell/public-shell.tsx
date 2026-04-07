"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import type { PublicShellSnapshot } from "../../lib/api/mock-data";
import { pickText, type UiLanguage, type UiTheme } from "../../lib/ui/preferences";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../ui/card";
import { ShellPreferences } from "./shell-preferences";
import { isNavHrefActive } from "./path-utils";

export function PublicShell({
  children,
  snapshot,
  lang,
  theme,
}: {
  children: ReactNode;
  snapshot: PublicShellSnapshot;
  lang: UiLanguage;
  theme: UiTheme | null;
}) {
  const pathname = usePathname();

  return (
    <div className="shell shell--public">
      <header className="shell-topbar shell-topbar--public">
        <div className="shell-topbar__copy">
          <Link className="brand-mark" href="/">
            {snapshot.brand}
          </Link>
          <p className="shell-topbar__subtitle">{snapshot.subtitle}</p>
        </div>
        <div className="shell-topbar__actions shell-topbar__actions--public">
          <nav aria-label={pickText(lang, "公共导航", "Public navigation")} className="shell-inline-nav">
            {snapshot.actions.map((item) => {
              const isActive = isNavHrefActive(pathname, item.href);

              return (
                <Link className={isActive ? "shell-link shell-link--active" : "shell-link"} href={item.href} key={item.href}>
                  {item.label}
                </Link>
              );
            })}
          </nav>
          <ShellPreferences lang={lang} theme={theme} />
        </div>
      </header>
      <div className="public-shell__layout">
        <aside className="public-shell__aside">
          <div className="hero-block">
            <p className="hero-block__eyebrow">{snapshot.eyebrow}</p>
            <h1>{snapshot.title}</h1>
            <p>{snapshot.description}</p>
          </div>
          <div className="stack-grid">
            {snapshot.highlights.map((item) => (
              <Card key={item.title} tone="accent">
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
              <CardTitle>{pickText(lang, "运行基线", "Operational Baseline")}</CardTitle>
              <CardDescription>{pickText(lang, "V1 保持计费和交易风险表面明确。", "V1 keeps billing and trading risk surfaces explicit.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {snapshot.supportLinks.map((item) => (
                  <li key={item.href}>
                    <Link href={item.href}>{item.label}</Link>
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
