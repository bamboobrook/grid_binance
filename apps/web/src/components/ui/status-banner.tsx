"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import { Chip, useUiCopy } from "./chip";

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

type BannerTone = string;

type Action = {
  href: string;
  label: string;
};

function resolveToneLabel(tone: BannerTone, copy: (zh: string, en: string) => string) {
  if (tone === "danger") {
    return copy("风险", "Risk");
  }
  if (tone === "warning") {
    return copy("警示", "Warning");
  }
  if (tone === "success") {
    return copy("在线", "Live");
  }
  return copy("信息", "Info");
}

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
  const copy = useUiCopy;
  const toneLabel = resolveToneLabel(tone, copy);

  return (
    <section aria-live="polite" className={cx("status-banner", `status-banner--${tone}`)} data-tone={tone} role={role}>
      <div className="status-banner__content">
        <div className="status-banner__meta">
          <Chip tone={tone}>{toneLabel}</Chip>
          <strong>{title}</strong>
        </div>
        <p>{description}</p>
        {extra ? <div className="status-banner__extra">{extra}</div> : null}
      </div>
      {action || extra ? (
        <div className="status-banner__actions">
          {action ? (
            <Link className="button button--ghost" href={action.href}>
              {action.label}
            </Link>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
