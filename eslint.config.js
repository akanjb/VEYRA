// @ts-check
import path from "node:path";

import js from "@eslint/js";
import { defineConfig, globalIgnores } from "eslint/config";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

import { veyraPlugin } from "./tooling/eslint/module-boundaries.js";

const desktopSrc = path.join(import.meta.dirname, "apps/desktop/src");

export default defineConfig(
  globalIgnores([
    "**/dist/",
    "**/coverage/",
    "target/",
    "src-tauri/gen/",
    "packages/contracts/src/generated/",
  ]),
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      // Review signals, not hard limits (docs/planning/02-architecture.md §3).
      "max-lines": ["warn", { max: 300, skipBlankLines: true, skipComments: true }],
      "max-lines-per-function": ["warn", { max: 60, skipBlankLines: true, skipComments: true }],
      "@typescript-eslint/consistent-type-imports": "error",
    },
  },
  {
    files: ["apps/desktop/src/**/*.{ts,tsx}"],
    extends: [reactHooks.configs.flat.recommended],
    languageOptions: { globals: globals.browser },
    plugins: { veyra: veyraPlugin },
    rules: { "veyra/module-boundaries": ["error", { srcRoot: desktopSrc }] },
  },
  {
    files: ["**/*.test.{ts,tsx,js}"],
    rules: { "max-lines-per-function": "off" },
  },
  {
    files: ["**/*.js"],
    languageOptions: { globals: globals.node },
  },
);
