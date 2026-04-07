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
        "rounded-sm border p-4 flex flex-col md:flex-row md:items-start gap-4 justify-between",
        {
          "bg-blue-500/10 border-blue-500/20 text-blue-400": tone === "info",
          "bg-emerald-500/10 border-emerald-500/20 text-emerald-400": tone === "success",
          "bg-amber-500/10 border-amber-500/20 text-amber-400": tone === "warning",
          "bg-red-500/10 border-red-500/20 text-red-400": tone === "danger",
        }
      )}
      role={tone === "danger" || tone === "warning" ? "alert" : "status"}
    >
      <div className="flex gap-3">
        <Icon className="w-5 h-5 shrink-0 mt-0.5" />
        <div className="space-y-1">
          <h4 className="text-sm font-bold text-slate-200">{title}</h4>
          <div className="text-xs opacity-90 leading-relaxed max-w-3xl">{description}</div>
          {extra && <div className="text-xs pt-2">{extra}</div>}
        </div>
      </div>
      {action && (
        <a 
          href={action.href}
          className={cn(
            "shrink-0 inline-flex items-center justify-center rounded-sm text-xs font-semibold px-3 py-1.5 transition-colors",
            {
              "bg-blue-500/20 hover:bg-blue-500/30 text-blue-300": tone === "info",
              "bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300": tone === "success",
              "bg-amber-500/20 hover:bg-amber-500/30 text-amber-300": tone === "warning",
              "bg-red-500/20 hover:bg-red-500/30 text-red-300": tone === "danger",
            }
          )}
        >
          {action.label}
        </a>
      )}
    </div>
  );
}
