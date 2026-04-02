import type { ReactNode } from "react";

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
  emptyMessage = "No records available.",
  rows,
}: {
  caption?: string;
  columns: readonly DataTableColumn[];
  emptyMessage?: string;
  rows: readonly DataTableRow[];
}) {
  return (
    <div className="table-wrap">
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
                {emptyMessage}
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
  );
}
