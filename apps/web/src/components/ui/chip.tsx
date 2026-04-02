import type { ReactNode } from "react";

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

type ChipTone = string;

export function Chip({
  children,
  className,
  tone = "default",
}: {
  children: ReactNode;
  className?: string;
  tone?: ChipTone;
}) {
  return <span className={cx("ui-chip", `ui-chip--${tone}`, className)}>{children}</span>;
}
