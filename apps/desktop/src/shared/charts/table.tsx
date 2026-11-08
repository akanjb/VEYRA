import type { KeyboardEvent, ReactNode } from "react";

import { cn } from "../ui";

export interface Column<T> {
  header: string;
  cell: (row: T) => ReactNode;
  numeric?: boolean;
}

/**
 * Props making a row behave like a button: focusable, opened by click, Enter or Space.
 * Rows without an action get none, so they stay out of the tab order.
 */
function rowAction(action: (() => void) | undefined) {
  if (action === undefined) return {};
  return {
    tabIndex: 0,
    onClick: action,
    onKeyDown: (event: KeyboardEvent<HTMLTableRowElement>) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      action();
    },
    className:
      // The global focus ring, drawn inside so the table's edge does not clip it.
      "cursor-pointer hover:bg-gray-3 focus-visible:bg-gray-3 focus-visible:-outline-offset-2",
  };
}

/** A plain, accessible table: the twin of every chart and the base of lists. */
export function DataTable<T>({
  columns,
  rows,
  rowKey,
  caption,
  onRowClick,
}: {
  columns: readonly Column<T>[];
  rows: readonly T[];
  rowKey: (row: T, index: number) => string;
  caption: string;
  onRowClick?: (row: T) => void;
}) {
  return (
    <table className="w-full border-collapse text-sm">
      <caption className="sr-only">{caption}</caption>
      <thead>
        <tr className="border-b border-border text-left text-xs text-subtle">
          {columns.map((c) => (
            <th
              key={c.header}
              scope="col"
              className={cn("px-2 py-2 font-medium", c.numeric === true && "text-right")}
            >
              {c.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, i) => {
          const { className, ...action } = rowAction(
            onRowClick === undefined
              ? undefined
              : () => {
                  onRowClick(row);
                },
          );
          return (
            <tr
              key={rowKey(row, i)}
              className={cn("border-b border-border/60 last:border-0", className)}
              {...action}
            >
              {columns.map((c) => (
                <td
                  key={c.header}
                  className={cn("px-2 py-1.5", c.numeric === true && "text-right tabular-nums")}
                >
                  {c.cell(row)}
                </td>
              ))}
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
