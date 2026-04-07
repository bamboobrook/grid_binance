"use client";

import Link from "next/link";

import { Chip, useUiCopy } from "./chip";

export type TabItem = {
  badge?: string;
  href: string;
  label: string;
};

export function Tabs({
  activeHref,
  items,
  label,
}: {
  activeHref: string;
  items: readonly TabItem[];
  label?: string;
}) {
  const resolvedLabel = label ?? useUiCopy("页面分区", "Page sections");

  return (
    <nav aria-label={resolvedLabel} className="ui-tabs">
      {items.map((item) => {
        const isActive = item.href === activeHref;

        return (
          <Link
            aria-current={isActive ? "page" : undefined}
            className={isActive ? "ui-tab ui-tab--active" : "ui-tab"}
            href={item.href}
            key={item.href}
          >
            <span className="ui-tab__label">{item.label}</span>
            {item.badge ? (
              <span className="ui-tab__meta">
                <Chip tone={isActive ? "warning" : "default"}>{item.badge}</Chip>
              </span>
            ) : null}
          </Link>
        );
      })}
    </nav>
  );
}
