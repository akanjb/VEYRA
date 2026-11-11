/**
 * A matrix as a table whose cells are shaded by value. It is a table first, so it needs
 * no table twin; the shading only adds a quick read of where the weight is.
 */
import { readableOn } from "./colors";
import { useChartTheme } from "./theme";

export interface HeatMatrix {
  rows: readonly string[];
  columns: readonly string[];
  /** `values[row][column]`. */
  values: readonly (readonly number[])[];
}

/** The ramp step for a value: 0 stays unshaded, the largest value takes the last step. */
function step(value: number, max: number, steps: number): number {
  if (value <= 0 || max <= 0) return -1;
  return Math.min(steps - 1, Math.floor((value / max) * (steps - 1)));
}

export function HeatTable({
  matrix,
  format,
  caption,
  corner,
}: {
  matrix: HeatMatrix;
  format: (v: number) => string;
  caption: string;
  /** Header of the row-label column, e.g. "Provider". */
  corner: string;
}) {
  const theme = useChartTheme();
  const ramp = theme.seq.slice(1);
  const max = Math.max(0, ...matrix.values.flat());
  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead>
          <tr className="border-b border-border text-left text-xs text-subtle">
            <th scope="col" className="px-2 py-2 font-medium">
              {corner}
            </th>
            {matrix.columns.map((column) => (
              <th key={column} scope="col" className="px-2 py-2 text-right font-medium">
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {matrix.rows.map((row, r) => (
            <tr key={`${row}-${String(r)}`} className="border-b border-border/60 last:border-0">
              <th scope="row" className="px-2 py-1.5 text-left font-normal break-all">
                {row}
              </th>
              {matrix.columns.map((column, c) => {
                const value = matrix.values[r]?.[c] ?? 0;
                const shade = ramp[step(value, max, ramp.length)];
                return (
                  <td
                    key={column}
                    ref={(cell) => {
                      // Through the CSSOM, which the production CSP allows (no inline styles).
                      if (cell === null) return;
                      cell.style.backgroundColor = shade ?? "";
                      cell.style.color = shade === undefined ? "" : readableOn(shade);
                    }}
                    className="px-2 py-1.5 text-right tabular-nums"
                  >
                    {format(value)}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
