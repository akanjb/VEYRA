// @ts-check
/**
 * Validates the categorical data palette in `apps/desktop/src/styles/tokens.css`
 * (docs/planning/03-ux.md §5.1), for the light and dark themes:
 *
 * - every data colour has at least 3:1 contrast against the card surface (WCAG 1.4.11);
 * - every pair is clearly distinct for normal vision (CIEDE2000 ΔE ≥ 10);
 * - the six most-used slots stay distinguishable under simulated protanopia,
 *   deuteranopia and tritanopia (Machado et al. 2009, full severity; ΔE ≥ 5).
 *
 * Charts never rely on colour alone (legends, labels and table twins carry meaning), so
 * the colour-blind check covers the slots that appear together most often.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const MIN_CONTRAST = 3;
export const MIN_DELTA_E = 10;
export const MIN_DELTA_E_CVD = 5;
export const CVD_SLOTS = 6;

/** @typedef {[number, number, number]} Rgb */

/** @param {string} hex */
export function parseHex(hex) {
  const m = /^#([0-9a-f]{6})$/i.exec(hex.trim());
  if (m?.[1] === undefined) throw new Error(`not a #rrggbb colour: ${hex}`);
  const n = Number.parseInt(m[1], 16);
  return /** @type {Rgb} */ ([(n >> 16) & 255, (n >> 8) & 255, n & 255]);
}

/** @param {number} c 0–255 */
const toLinear = (c) => {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};
/** @param {number} l linear 0–1 */
const toSrgb = (l) => {
  const v = Math.min(1, Math.max(0, l));
  return 255 * (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
};

/** @param {Rgb} rgb */
export function luminance([r, g, b]) {
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
}

/** @param {Rgb} a @param {Rgb} b */
export function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return ((hi ?? 0) + 0.05) / ((lo ?? 0) + 0.05);
}

/** @param {Rgb} rgb @returns {[number, number, number]} */
function toLab([r, g, b]) {
  const [lr, lg, lb] = [toLinear(r), toLinear(g), toLinear(b)];
  const x = (0.4124 * lr + 0.3576 * lg + 0.1805 * lb) / 0.95047;
  const y = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
  const z = (0.0193 * lr + 0.1192 * lg + 0.9505 * lb) / 1.08883;
  /** @param {number} t */
  const f = (t) => (t > 216 / 24389 ? Math.cbrt(t) : ((24389 / 27) * t + 16) / 116);
  const [fx, fy, fz] = [f(x), f(y), f(z)];
  return [116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)];
}

/** CIEDE2000 colour difference. @param {Rgb} a @param {Rgb} b */
export function deltaE2000(a, b) {
  const [l1, a1, b1] = toLab(a);
  const [l2, a2, b2] = toLab(b);
  const rad = Math.PI / 180;
  const c1 = Math.hypot(a1, b1);
  const c2 = Math.hypot(a2, b2);
  const cBar = (c1 + c2) / 2;
  const g = 0.5 * (1 - Math.sqrt(cBar ** 7 / (cBar ** 7 + 25 ** 7)));
  const ap1 = (1 + g) * a1;
  const ap2 = (1 + g) * a2;
  const cp1 = Math.hypot(ap1, b1);
  const cp2 = Math.hypot(ap2, b2);
  /** @param {number} y @param {number} x */
  const hue = (y, x) => (x === 0 && y === 0 ? 0 : (Math.atan2(y, x) / rad + 360) % 360);
  const hp1 = hue(b1, ap1);
  const hp2 = hue(b2, ap2);
  const dL = l2 - l1;
  const dC = cp2 - cp1;
  let dh = hp2 - hp1;
  if (cp1 * cp2 === 0) dh = 0;
  else if (dh > 180) dh -= 360;
  else if (dh < -180) dh += 360;
  const dH = 2 * Math.sqrt(cp1 * cp2) * Math.sin((dh / 2) * rad);
  const lBar = (l1 + l2) / 2;
  const cpBar = (cp1 + cp2) / 2;
  let hBar = hp1 + hp2;
  if (cp1 * cp2 !== 0) {
    if (Math.abs(hp1 - hp2) > 180) hBar += hBar < 360 ? 360 : -360;
    hBar /= 2;
  }
  const t =
    1 -
    0.17 * Math.cos((hBar - 30) * rad) +
    0.24 * Math.cos(2 * hBar * rad) +
    0.32 * Math.cos((3 * hBar + 6) * rad) -
    0.2 * Math.cos((4 * hBar - 63) * rad);
  const sl = 1 + (0.015 * (lBar - 50) ** 2) / Math.sqrt(20 + (lBar - 50) ** 2);
  const sc = 1 + 0.045 * cpBar;
  const sh = 1 + 0.015 * cpBar * t;
  const rc = 2 * Math.sqrt(cpBar ** 7 / (cpBar ** 7 + 25 ** 7));
  const rt = -Math.sin(2 * 30 * Math.exp(-(((hBar - 275) / 25) ** 2)) * rad) * rc;
  return Math.sqrt((dL / sl) ** 2 + (dC / sc) ** 2 + (dH / sh) ** 2 + rt * (dC / sc) * (dH / sh));
}

/** Machado et al. (2009) full-severity simulation matrices, in linear RGB. */
const CVD = {
  protanopia: [
    0.152286, 1.052583, -0.204868, 0.114503, 0.786281, 0.099216, -0.003882, -0.048116, 1.051998,
  ],
  deuteranopia: [
    0.367322, 0.860646, -0.227968, 0.280085, 0.672501, 0.047413, -0.01182, 0.04294, 0.968881,
  ],
  tritanopia: [
    1.255528, -0.076749, -0.178779, -0.078411, 0.930809, 0.147602, 0.004733, 0.691367, 0.3039,
  ],
};

/** @param {Rgb} rgb @param {keyof typeof CVD} kind @returns {Rgb} */
export function simulate([r, g, b], kind) {
  const m = CVD[kind];
  const [lr, lg, lb] = [toLinear(r), toLinear(g), toLinear(b)];
  /** @param {number} i */
  const row = (i) => toSrgb((m[i] ?? 0) * lr + (m[i + 1] ?? 0) * lg + (m[i + 2] ?? 0) * lb);
  return [row(0), row(3), row(6)];
}

/**
 * Extracts `--name: #hex` declarations from the block that follows `selector`.
 * @param {string} css @param {string} selector
 */
export function variables(css, selector) {
  const start = css.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`no ${selector} block`);
  const block = css.slice(start, css.indexOf("}", start));
  /** @type {Record<string, string>} */
  const vars = {};
  for (const m of block.matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) {
    if (m[1] !== undefined && m[2] !== undefined) vars[m[1]] = m[2];
  }
  return vars;
}

/**
 * Checks one theme; returns human-readable failures.
 * @param {Record<string, string>} vars @param {string} theme
 */
export function checkTheme(vars, theme) {
  const failures = [];
  const surface = vars["surface"];
  if (surface === undefined) return [`${theme}: no --surface`];
  const slots = [];
  for (let i = 1; vars[`data-${String(i)}`] !== undefined; i += 1) {
    slots.push({ name: `data-${String(i)}`, rgb: parseHex(vars[`data-${String(i)}`] ?? "") });
  }
  if (slots.length !== 10)
    failures.push(`${theme}: expected 10 data slots, found ${String(slots.length)}`);
  for (const slot of slots) {
    const ratio = contrast(slot.rgb, parseHex(surface));
    if (ratio < MIN_CONTRAST)
      failures.push(
        `${theme}: ${slot.name} contrast ${ratio.toFixed(2)} < ${String(MIN_CONTRAST)}`,
      );
  }
  for (let i = 0; i < slots.length; i += 1) {
    for (let j = i + 1; j < slots.length; j += 1) {
      const [a, b] = [slots[i], slots[j]];
      if (a === undefined || b === undefined) continue;
      const d = deltaE2000(a.rgb, b.rgb);
      if (d < MIN_DELTA_E)
        failures.push(`${theme}: ${a.name}/${b.name} ΔE ${d.toFixed(1)} < ${String(MIN_DELTA_E)}`);
      if (i < CVD_SLOTS && j < CVD_SLOTS) {
        for (const kind of /** @type {(keyof typeof CVD)[]} */ (Object.keys(CVD))) {
          const dc = deltaE2000(simulate(a.rgb, kind), simulate(b.rgb, kind));
          if (dc < MIN_DELTA_E_CVD) {
            failures.push(
              `${theme}: ${a.name}/${b.name} under ${kind} ΔE ${dc.toFixed(1)} < ${String(MIN_DELTA_E_CVD)}`,
            );
          }
        }
      }
    }
  }
  return failures;
}

/** @param {string} css */
export function checkPalette(css) {
  return [
    ...checkTheme(variables(css, ":root"), "light"),
    ...checkTheme(variables(css, ':root[data-theme="dark"]'), "dark"),
  ];
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const file = new URL("../../apps/desktop/src/styles/tokens.css", import.meta.url);
  const failures = checkPalette(readFileSync(file, "utf8"));
  if (failures.length > 0) {
    console.error(`Palette check failed:\n  ${failures.join("\n  ")}`);
    process.exit(1);
  }
  console.log("Palette check passed (contrast, separation, colour-blind simulation).");
}
