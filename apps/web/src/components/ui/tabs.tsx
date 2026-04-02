import Link from "next/link";

import { Chip } from "./chip";

export type TabItem = {
  badge?: string;
  href: string;
  label: string;
};

export function Tabs({
  activeHref,
  items,
  label = "Page sections",
}: {
  activeHref: string;
  items: readonly TabItem[];
  label?: string;
}) {
  return (
    <nav aria-label={label} className="ui-tabs">
      {items.map((item) => {
        const isActive = item.href === activeHref;

        return (
          <Link
            aria-current={isActive ? "page" : undefined}
            className={isActive ? "ui-tab ui-tab--active" : "ui-tab"}
            href={item.href}
            key={item.href}
          >
            <span>{item.label}</span>
            {item.badge ? <Chip tone={isActive ? "info" : "default"}>{item.badge}</Chip> : null}
          </Link>
        );
      })}
    </nav>
  );
}
