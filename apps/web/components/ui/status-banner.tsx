import { cn } from "@/lib/utils";
import type { ReactNode } from "react";
import { AlertTriangle, Info, CheckCircle, XCircle } from "lucide-react";

type BannerTone = "info" | "success" | "warning" | "danger";

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
  const Icon = tone === "danger" ? XCircle : tone === "warning" ? AlertTriangle : tone === "success" ? CheckCircle : Info;

  return (
    <div
      className={cn(
        "status-banner rounded-sm border p-4 flex flex-col gap-4 justify-between md:flex-row md:items-start",
        {
          "bg-blue-50 border-blue-200 text-foreground dark:bg-blue-500/10 dark:border-blue-500/20": tone === "info",
          "bg-emerald-50 border-emerald-200 text-foreground dark:bg-emerald-500/10 dark:border-emerald-500/20": tone === "success",
          "bg-amber-50 border-amber-200 text-foreground dark:bg-amber-500/10 dark:border-amber-500/20": tone === "warning",
          "bg-red-50 border-red-200 text-foreground dark:bg-red-500/10 dark:border-red-500/20": tone === "danger",
        }
      )}
      role={tone === "danger" || tone === "warning" ? "alert" : "status"}
    >
      <div className="status-banner__meta flex gap-3">
        <Icon
          className={cn("w-5 h-5 shrink-0 mt-0.5", {
            "text-blue-700 dark:text-blue-300": tone === "info",
            "text-emerald-700 dark:text-emerald-300": tone === "success",
            "text-amber-700 dark:text-amber-300": tone === "warning",
            "text-red-700 dark:text-red-300": tone === "danger",
          })}
        />
        <div className="status-banner__copy space-y-1">
          <h4 className="text-sm font-bold text-foreground">{title}</h4>
          <div className="max-w-3xl text-xs leading-relaxed text-muted-foreground">{description}</div>
          {extra && <div className="pt-2 text-xs text-muted-foreground">{extra}</div>}
        </div>
      </div>
      {action && (
        <a 
          href={action.href}
          className={cn(
            "status-banner__actions shrink-0 inline-flex items-center justify-center rounded-sm px-3 py-1.5 text-xs font-semibold transition-colors",
            {
              "bg-blue-100 hover:bg-blue-200 text-blue-900 dark:bg-blue-500/20 dark:hover:bg-blue-500/30 dark:text-blue-100": tone === "info",
              "bg-emerald-100 hover:bg-emerald-200 text-emerald-900 dark:bg-emerald-500/20 dark:hover:bg-emerald-500/30 dark:text-emerald-100": tone === "success",
              "bg-amber-100 hover:bg-amber-200 text-amber-900 dark:bg-amber-500/20 dark:hover:bg-amber-500/30 dark:text-amber-100": tone === "warning",
              "bg-red-100 hover:bg-red-200 text-red-900 dark:bg-red-500/20 dark:hover:bg-red-500/30 dark:text-red-100": tone === "danger",
            }
          )}
        >
          {action.label}
        </a>
      )}
    </div>
  );
}
