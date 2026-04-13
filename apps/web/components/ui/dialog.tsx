import type { ReactNode } from "react";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

import { Chip } from "./chip";

function resolveToneLabel(tone: "info" | "warning" | "danger", lang: UiLanguage) {
  if (tone === "warning") {
    return pickText(lang, "警示", "Warning");
  }
  if (tone === "danger") {
    return pickText(lang, "严重", "Critical");
  }
  return pickText(lang, "提示", "Heads up");
}

export function DialogFrame({
  children,
  description,
  lang = "en",
  title,
  tone = "info",
  modal = false,
}: {
  children?: ReactNode;
  description: string;
  lang?: UiLanguage;
  title: string;
  tone?: "info" | "warning" | "danger";
  modal?: boolean;
}) {
  const toneLabel = resolveToneLabel(tone, lang);

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
