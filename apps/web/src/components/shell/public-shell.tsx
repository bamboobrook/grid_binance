import Link from "next/link";
import type { ReactNode } from "react";

import type { PublicShellSnapshot } from "../../lib/api/mock-data";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../ui/card";

export function PublicShell({
  children,
  snapshot,
}: {
  children: ReactNode;
  snapshot: PublicShellSnapshot;
}) {
  return (
    <div className="shell shell--public">
      <header className="shell-topbar shell-topbar--public">
        <div>
          <Link className="brand-mark" href="/">
            {snapshot.brand}
          </Link>
          <p className="shell-topbar__subtitle">{snapshot.subtitle}</p>
        </div>
        <nav aria-label="Public navigation" className="shell-inline-nav">
          {snapshot.actions.map((item) => (
            <Link href={item.href} key={item.href}>
              {item.label}
            </Link>
          ))}
        </nav>
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
              <CardTitle>Operational baseline</CardTitle>
              <CardDescription>V1 keeps billing and trading risk surfaces explicit.</CardDescription>
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
