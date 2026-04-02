import type { ReactNode } from "react";

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

export function AppShellSection({
  actions,
  children,
  className,
  description,
  eyebrow,
  title,
}: {
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  description?: string;
  eyebrow?: string;
  title: string;
}) {
  return (
    <section className={cx("app-section", className)}>
      <header className="app-section__header">
        <div>
          {eyebrow ? <p className="app-section__eyebrow">{eyebrow}</p> : null}
          <h1 className="app-section__title">{title}</h1>
          {description ? <p className="app-section__description">{description}</p> : null}
        </div>
        {actions ? <div className="app-section__actions">{actions}</div> : null}
      </header>
      {children}
    </section>
  );
}
