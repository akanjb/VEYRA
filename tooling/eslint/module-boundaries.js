// @ts-check
/**
 * ESLint rule enforcing the dashboard's architectural boundaries
 * (docs/planning/02-architecture.md §3):
 *
 * - `shared/*` must not import from `modules/*` or `app/*`.
 * - `modules/<a>` must not import from `modules/<b>` or `app/*`.
 * - Only `shared/api` may import Tauri packages (`@tauri-apps/*`).
 *
 * Imports are resolved to an "area" of the source tree, so relative and alias (`@/`)
 * imports are treated identically.
 */
import path from "node:path";

/** @typedef {{ kind: "app" } | { kind: "shared", area: string } | { kind: "module", name: string } | { kind: "other" }} Area */

/**
 * Classifies a path relative to the source root.
 * @param {string} relative POSIX path relative to the source root
 * @returns {Area}
 */
export function classify(relative) {
  const [top, second] = relative.split("/");
  if (top === "app") return { kind: "app" };
  if (top === "shared" && second !== undefined) return { kind: "shared", area: second };
  if (top === "modules" && second !== undefined) return { kind: "module", name: second };
  return { kind: "other" };
}

/**
 * Resolves an import specifier to a path relative to the source root, or `null` when it
 * refers to an external package.
 * @param {string} specifier
 * @param {string} importerRelative POSIX path of the importing file relative to the source root
 */
export function resolveRelative(specifier, importerRelative) {
  if (specifier.startsWith("@/")) return specifier.slice(2);
  if (specifier.startsWith(".")) {
    return path.posix.normalize(path.posix.join(path.posix.dirname(importerRelative), specifier));
  }
  return null;
}

/**
 * Returns a violation message, or `null` when the import is allowed.
 * @param {Area} from
 * @param {Area} to
 */
export function boundaryViolation(from, to) {
  if (from.kind === "shared" && (to.kind === "module" || to.kind === "app")) {
    return "shared/ code must not depend on app/ or product modules.";
  }
  if (from.kind === "module" && to.kind === "app") {
    return "Modules must not depend on app/; receive what you need via props or shared/.";
  }
  if (from.kind === "module" && to.kind === "module" && from.name !== to.name) {
    return `Module "${from.name}" must not import module "${to.name}"; move shared code to shared/.`;
  }
  return null;
}

/** @type {import("eslint").Rule.RuleModule} */
const rule = {
  meta: {
    type: "problem",
    docs: { description: "Enforce dashboard module boundaries" },
    schema: [
      {
        type: "object",
        properties: { srcRoot: { type: "string" } },
        required: ["srcRoot"],
        additionalProperties: false,
      },
    ],
  },
  create(context) {
    /** @type {unknown} */
    const options = context.options[0];
    const srcRoot =
      typeof options === "object" && options !== null && "srcRoot" in options
        ? options.srcRoot
        : undefined;
    if (typeof srcRoot !== "string") throw new Error("module-boundaries: `srcRoot` is required");
    const importer = path.relative(srcRoot, context.filename).split(path.sep).join("/");
    if (importer.startsWith("..")) return {};
    const from = classify(importer);

    /** @param {import("estree").Node & { source?: unknown }} node */
    function check(node) {
      const source = /** @type {{ value?: unknown } | null | undefined} */ (node.source);
      if (typeof source?.value !== "string") return;
      const specifier = source.value;

      if (specifier.startsWith("@tauri-apps/")) {
        if (!(from.kind === "shared" && from.area === "api")) {
          context.report({ node, message: "Only shared/api may import Tauri packages." });
        }
        return;
      }

      const target = resolveRelative(specifier, importer);
      if (target === null) return;
      const message = boundaryViolation(from, classify(target));
      if (message !== null) context.report({ node, message });
    }

    return {
      ImportDeclaration: check,
      ExportNamedDeclaration: check,
      ExportAllDeclaration: check,
      ImportExpression: check,
    };
  },
};

/** @type {import("eslint").ESLint.Plugin} */
export const veyraPlugin = { rules: { "module-boundaries": rule } };
