import Link from "next/link";
import type { ReactNode } from "react";

import type { AdminShellSnapshot } from "../../lib/api/mock-data";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../ui/card";
import { Chip } from "../ui/chip";
import { StatusBanner } from "../ui/status-banner";

export function AdminShell({
  children,
  snapshot,
}: {
  children: ReactNode;
  snapshot: AdminShellSnapshot;
}) {
  return (
    <div className="shell shell--workspace shell--admin">
      <aside className="shell-sidebar shell-sidebar--admin">
        <div className="shell-sidebar__brand">
          <Link className="brand-mark" href="/admin/dashboard">
            {snapshot.brand}
          </Link>
          <p>{snapshot.subtitle}</p>
        </div>
        <nav aria-label="Admin workspace" className="shell-sidebar__nav">
          {snapshot.nav.map((item) => {
            const isActive = item.href === snapshot.activeHref;
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
          <div>
            <p className="shell-topbar__eyebrow">Admin operations</p>
            <h1>{snapshot.title}</h1>
            <p className="shell-topbar__subtitle">{snapshot.description}</p>
          </div>
          <div className="metric-strip">
            {snapshot.quickStats.map((item) => (
              <div className="metric-strip__item" key={item.label}>
                <span>{item.label}</span>
                <strong>{item.value}</strong>
              </div>
            ))}
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
