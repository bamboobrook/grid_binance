"use client";

import type { ReactNode } from "react";

import { useUiCopy } from "./chip";

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

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
  const resolvedEmptyMessage = emptyMessage ?? useUiCopy("暂无记录。", "No matching records.");

  return (
    <div className="table-wrap">
      <div className="ui-table__scroller">
        <table className="ui-table">
          {caption ? <caption>{caption}</caption> : null}
          <thead>
            <tr>
              {columns.map((column) => (
                <th className={cx(column.align && `is-${column.align}`)} key={column.key} scope="col">
                  {column.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td className="ui-table__empty" colSpan={columns.length}>
                  {resolvedEmptyMessage}
                </td>
              </tr>
            ) : null}
            {rows.map((row) => (
              <tr key={row.id}>
                {columns.map((column) => (
                  <td className={cx(column.align && `is-${column.align}`)} key={column.key}>
                    {row[column.key] ?? "-"}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
