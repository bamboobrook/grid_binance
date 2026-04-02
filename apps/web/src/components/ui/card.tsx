import type { ReactNode } from "react";

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

type CardTone = "default" | "accent" | "subtle";

type CardProps = {
  children: ReactNode;
  className?: string;
  tone?: CardTone;
};

type SlotProps = {
  children: ReactNode;
  className?: string;
};

export function Card({ children, className, tone = "default" }: CardProps) {
  return <section className={cx("ui-card", tone !== "default" && `ui-card--${tone}`, className)}>{children}</section>;
}

export function CardHeader({ children, className }: SlotProps) {
  return <header className={cx("ui-card__header", className)}>{children}</header>;
}

export function CardTitle({ children, className }: SlotProps) {
  return <h2 className={cx("ui-card__title", className)}>{children}</h2>;
}

export function CardDescription({ children, className }: SlotProps) {
  return <p className={cx("ui-card__description", className)}>{children}</p>;
}

export function CardBody({ children, className }: SlotProps) {
  return <div className={cx("ui-card__body", className)}>{children}</div>;
}

export function CardFooter({ children, className }: SlotProps) {
  return <footer className={cx("ui-card__footer", className)}>{children}</footer>;
}
