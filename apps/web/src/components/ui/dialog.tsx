import type { ReactNode } from "react";

import { Chip } from "./chip";

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
  return (
    <section aria-modal={modal ? "true" : "false"} className={`ui-dialog ui-dialog--${tone}`} role="dialog">
      <header className="ui-dialog__header">
        <Chip tone={tone}>{tone === "info" ? "Heads up" : tone === "warning" ? "Warning" : "Critical"}</Chip>
        <h3>{title}</h3>
      </header>
      <p>{description}</p>
      {children ? <div className="ui-dialog__body">{children}</div> : null}
    </section>
  );
}
