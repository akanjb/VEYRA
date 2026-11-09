/**
 * Chart tooltip content, built as DOM nodes rather than HTML strings:
 * - text is set with `textContent`, so values are never parsed as markup and need no escaping;
 * - colours are set through the CSSOM, which the production Content Security Policy allows
 *   (it blocks inline `style` attributes, such as those of ECharts' default markers).
 */

export interface TooltipLine {
  label: string;
  value?: string;
  /** A colour swatch identifying the series. */
  color?: string;
}

function text(tag: string, content: string, className?: string): HTMLElement {
  const node = document.createElement(tag);
  node.textContent = content;
  if (className !== undefined) node.className = className;
  return node;
}

function line({ label, value, color }: TooltipLine): HTMLElement {
  const row = document.createElement("div");
  row.className = "flex items-center gap-2";
  if (color !== undefined) {
    const swatch = document.createElement("span");
    swatch.className = "size-2.5 shrink-0 rounded-full";
    swatch.style.backgroundColor = color;
    row.append(swatch);
  }
  row.append(text("span", label));
  if (value !== undefined) row.append(text("b", value, "ml-auto pl-4 tabular-nums"));
  return row;
}

/** A bold title followed by one line per item. */
export function tooltipContent(title: string, lines: readonly TooltipLine[]): HTMLElement {
  const root = document.createElement("div");
  root.className = "grid gap-0.5";
  root.append(text("b", title), ...lines.map(line));
  return root;
}
