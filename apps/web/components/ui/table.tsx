import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

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
  const resolvedEmptyMessage = emptyMessage ?? "No matching records.";

  return (
    <div className="w-full overflow-x-auto rounded-sm border border-slate-800 bg-[#131b2c]">
      <table className="w-full text-left text-sm">
        {caption ? (
          <caption className="text-xs text-slate-500 font-bold uppercase tracking-wider text-left p-4 pb-2">
            {caption}
          </caption>
        ) : null}
        <thead className="bg-[#0a101d] text-slate-500 text-[10px] uppercase tracking-wider border-b border-slate-800">
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
        <tbody className="divide-y divide-slate-800/50">
          {rows.length === 0 ? (
            <tr>
              <td className="px-4 py-8 text-center text-xs text-slate-500" colSpan={columns.length}>
                {resolvedEmptyMessage}
              </td>
            </tr>
          ) : null}
          {rows.map((row) => (
            <tr key={row.id} className="hover:bg-slate-800/30 transition-colors">
              {columns.map((column) => (
                <td 
                  className={cn("px-4 py-3 text-xs text-slate-300 font-mono", {
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
