import { cn } from "@/lib/utils";
import type { ReactNode } from "react";
import { pickText, resolveUiLanguage } from "@/lib/ui/preferences";

export type DataTableColumn = {
  align?: "left" | "right" | "center";
  key: string;
  label: string;
};

export type DataTableRow = Record<string, ReactNode> & {
  id: string;
};

export function DataTable({
  caption,
  columns,
  emptyMessage,
  rows,
}: {
  caption?: string;
  columns: readonly DataTableColumn[];
  emptyMessage?: string;
  rows: readonly DataTableRow[];
}) {
  const resolvedEmptyMessage = emptyMessage ?? defaultEmptyMessage();

  return (
    <div className="ui-table__scroller w-full overflow-x-auto rounded-sm border border-border bg-card">
      <table className="ui-table w-full text-left text-sm">
        {caption ? (
          <caption className="text-xs text-muted-foreground font-bold uppercase tracking-wider text-left p-4 pb-2">
            {caption}
          </caption>
        ) : null}
        <thead className="bg-muted text-muted-foreground text-[10px] uppercase tracking-wider border-b border-border">
          <tr>
            {columns.map((column) => (
              <th 
                className={cn("px-4 py-2 font-medium", {
                  "text-right": column.align === "right",
                  "text-center": column.align === "center",
                })} 
                key={column.key} 
                scope="col"
              >
                {column.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {rows.length === 0 ? (
            <tr>
              <td className="px-4 py-8 text-center text-xs text-muted-foreground" colSpan={columns.length}>
                {resolvedEmptyMessage}
              </td>
            </tr>
          ) : null}
          {rows.map((row) => (
            <tr key={row.id} className="hover:bg-secondary/30 transition-colors">
              {columns.map((column) => (
                <td 
                  className={cn("px-4 py-3 text-sm text-foreground align-top", {
                    "text-right": column.align === "right",
                    "text-center": column.align === "center",
                  })} 
                  key={column.key}
                >
                  {row[column.key] ?? "-"}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function defaultEmptyMessage() {
  const lang = typeof document !== "undefined"
    ? resolveUiLanguage(document.documentElement.lang)
    : "zh";
  return pickText(lang, "暂无匹配记录。", "No matching records.");
}
