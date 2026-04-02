import Link from "next/link";
import type { ReactNode } from "react";

import { Chip } from "./chip";

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

type BannerTone = string;

type Action = {
  href: string;
  label: string;
};

export function StatusBanner({
  action,
  description,
  extra,
  title,
  tone = "info",
}: {
  action?: Action;
  description: ReactNode;
  extra?: ReactNode;
  title: string;
  tone?: BannerTone;
}) {
  const role = tone === "danger" || tone === "warning" ? "alert" : "status";

  return (
    <section aria-live="polite" className={cx("status-banner", `status-banner--${tone}`)} role={role}>
      <div className="status-banner__content">
        <div className="status-banner__heading">
          <Chip tone={tone}>{tone.toUpperCase()}</Chip>
          <strong>{title}</strong>
        </div>
        <p>{description}</p>
        {extra ? <div className="status-banner__extra">{extra}</div> : null}
      </div>
      {action ? (
        <Link className="button button--ghost" href={action.href}>
          {action.label}
        </Link>
      ) : null}
    </section>
  );
}
