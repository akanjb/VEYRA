// @ts-check
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { fileFor, headerValue, productionCsp, withStyleNonce } from "./serve-with-csp.js";

const CONF = fileURLToPath(new URL("../../src-tauri/tauri.conf.json", import.meta.url));

describe("serve with the production CSP", () => {
  it("builds the header from tauri.conf.json, without the dev relaxations", () => {
    const csp = headerValue(productionCsp(CONF));
    expect(csp).toContain("default-src 'self'");
    expect(csp).toContain("style-src 'self'");
    expect(csp).not.toContain("unsafe-inline");
  });

  it("gives each style element a nonce and allows only that nonce, like Tauri", () => {
    const page = '<head><style></style><style nonce="kept"></style></head>';
    const served = withStyleNonce(page, { "style-src": "'self'" }, "n1");
    expect(served.html).toBe('<head><style nonce="n1"></style><style nonce="kept"></style></head>');
    expect(served.directives["style-src"]).toBe("'self' 'nonce-n1'");
    // Pages without style elements keep the policy unchanged.
    expect(withStyleNonce("<p></p>", { "style-src": "'self'" }, "n2").directives).toEqual({
      "style-src": "'self'",
    });
  });

  it("serves files inside the root, routes to index.html and refuses escapes", () => {
    const root = resolve("dist-test");
    expect(fileFor(root, "/assets/app.js")).toBe(`${root}${sep}assets${sep}app.js`);
    expect(fileFor(root, "/sessions/12")).toBe(`${root}${sep}index.html`);
    expect(fileFor(root, "/../secret.txt")).toBeNull();
    expect(fileFor(root, "/%2e%2e/secret.txt")).toBeNull();
  });
});
