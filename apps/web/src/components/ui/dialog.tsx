"use client";

import type { ReactNode } from "react";

import { Chip, useUiCopy } from "./chip";

function resolveToneLabel(tone: "info" | "warning" | "danger", copy: (zh: string, en: string) => string) {
  if (tone === "warning") {
    return copy("警示", "Warning");
  }
  if (tone === "danger") {
    return copy("严重", "Critical");
  }
  return copy("提示", "Heads up");
}

export function DialogFrame({
  children,
  description,
  title,
  tone = "info",
  modal = false,
}: {
  children?: ReactNode;
  description: string;
  title: string;
  tone?: "info" | "warning" | "danger";
  modal?: boolean;
}) {
  const copy = useUiCopy;
  const toneLabel = resolveToneLabel(tone, copy);

  return (
    <section aria-modal={modal ? "true" : "false"} className={`ui-dialog ui-dialog--${tone}`} role="dialog">
      <header className="ui-dialog__header">
        <Chip tone={tone}>{toneLabel}</Chip>
        <div className="ui-dialog__copy">
          <h3>{title}</h3>
          <p>{description}</p>
        </div>
      </header>
      {children ? <div className="ui-dialog__body">{children}</div> : null}
    </section>
  );
}
