// @ts-check
import path from "node:path";

import { Linter } from "eslint";
import { describe, expect, it } from "vitest";

import { veyraPlugin } from "./module-boundaries.js";

// Must live under the working directory, or flat config treats the file as unmatched.
const srcRoot = path.resolve("apps/desktop/src");

/** @type {(from: string, to: string) => string} */
const CROSS_MODULE = (from, to) =>
  `Module "${from}" must not import module "${to}"; move shared code to shared/.`;
const SHARED_UPWARD = "shared/ code must not depend on app/ or product modules.";
const MODULE_TO_APP =
  "Modules must not depend on app/; receive what you need via props or shared/.";
const TAURI_OUTSIDE_API = "Only shared/api may import Tauri packages.";

/**
 * Lints `code` as if it lived at `relativeFile` under the source root and returns the
 * reported messages.
 * @param {string} relativeFile
 * @param {string} code
 */
function lint(relativeFile, code) {
  const linter = new Linter({ configType: "flat" });
  return linter
    .verify(
      code,
      [
        {
          files: ["**/*.js"],
          plugins: { veyra: veyraPlugin },
          rules: { "veyra/module-boundaries": ["error", { srcRoot }] },
        },
      ],
      { filename: path.join(srcRoot, relativeFile) },
    )
    .map((m) => m.message);
}

describe("veyra/module-boundaries", () => {
  it("allows modules to import shared code and their own files", () => {
    const code =
      'import { x } from "@/shared/format/numbers"; import { y } from "./components/hero";';
    expect(lint("modules/overview/overview.route.js", code)).toEqual([]);
  });

  it("rejects a module importing another module (alias and relative)", () => {
    const expected = [CROSS_MODULE("overview", "usage")];
    expect(lint("modules/overview/a.js", 'import { x } from "@/modules/usage/b";')).toEqual(
      expected,
    );
    expect(lint("modules/overview/a.js", 'import { x } from "../usage/b";')).toEqual(expected);
  });

  it("rejects shared code importing modules or app", () => {
    expect(lint("shared/charts/trend.js", 'import { x } from "@/modules/cost/c";')).toEqual([
      SHARED_UPWARD,
    ]);
    expect(lint("shared/charts/trend.js", 'import { x } from "../../app/shell";')).toEqual([
      SHARED_UPWARD,
    ]);
  });

  it("rejects modules importing app", () => {
    expect(lint("modules/cost/c.js", 'import { x } from "@/app/shell";')).toEqual([MODULE_TO_APP]);
  });

  it("allows Tauri imports only inside shared/api", () => {
    const code = 'import { invoke } from "@tauri-apps/api/core";';
    expect(lint("shared/api/tauri-api-client.js", code)).toEqual([]);
    expect(lint("modules/overview/a.js", code)).toEqual([TAURI_OUTSIDE_API]);
    expect(lint("app/main.js", code)).toEqual([TAURI_OUTSIDE_API]);
  });

  it("checks re-exports and dynamic imports too", () => {
    expect(lint("shared/ui/index.js", 'export * from "@/modules/x/y";')).toEqual([SHARED_UPWARD]);
    expect(lint("modules/a/z.js", 'const m = import("@/modules/b/y");')).toEqual([
      CROSS_MODULE("a", "b"),
    ]);
  });

  it("allows the app layer to import shared code and modules", () => {
    const code = 'import { x } from "@/shared/api"; import { y } from "@/modules/overview/route";';
    expect(lint("app/router.js", code)).toEqual([]);
  });
});
