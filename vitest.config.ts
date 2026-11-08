import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    projects: [
      "apps/desktop",
      {
        test: {
          name: "tooling",
          include: ["tooling/**/*.test.js"],
          environment: "node",
        },
      },
    ],
  },
});
