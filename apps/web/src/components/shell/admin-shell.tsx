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
import { isNavHrefActive } from "./path-utils";

export function AdminShell({
  children,
  snapshot,
  lang,
  theme,
}: {
  children: ReactNode;
  snapshot: AdminShellSnapshot;
  lang: UiLanguage;
  theme: UiTheme | null;
}) {
  const pathname = usePathname();

  return (
    <div className="shell shell--workspace shell--admin">
      <aside className="shell-sidebar shell-sidebar--admin">
        <div className="shell-sidebar__brand">
          <Link className="brand-mark" href="/admin/dashboard">
            {snapshot.brand}
          </Link>
          <p>{snapshot.subtitle}</p>
        </div>
        <nav aria-label={pickText(lang, "管理员导航", "Admin workspace")} className="shell-sidebar__nav">
          {snapshot.nav.map((item) => {
            const isActive = isNavHrefActive(pathname, item.href);
            return (
              <Link className={isActive ? "shell-link shell-link--active" : "shell-link"} href={item.href} key={item.href}>
                <span>{item.label}</span>
                {item.badge ? <Chip tone={isActive ? "warning" : "default"}>{item.badge}</Chip> : null}
              </Link>
            );
          })}
        </nav>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>{snapshot.identity.name}</CardTitle>
            <CardDescription>{snapshot.identity.role}</CardDescription>
          </CardHeader>
          <CardBody>
            <p>{snapshot.identity.context}</p>
          </CardBody>
        </Card>
      </aside>
      <div className="shell-content">
        <header className="shell-topbar">
          <div className="shell-topbar__copy">
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
              action={banner.action}
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
