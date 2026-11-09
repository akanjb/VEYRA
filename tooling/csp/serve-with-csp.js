// @ts-check
/**
 * Serves a built dashboard with the production Content Security Policy of
 * `src-tauri/tauri.conf.json`, so UI tests can catch what only the strict policy
 * blocks (inline style attributes, inline `<style>` elements, `data:` fonts). Like
 * Tauri, it gives each page's `<style>` elements a fresh nonce.
 *
 * Usage: node tooling/csp/serve-with-csp.js <dist directory> <port>
 */
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const TYPES = /** @type {Record<string, string>} */ ({
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
});

/**
 * The production policy's directives.
 * @param {string} confPath path of tauri.conf.json
 * @returns {Record<string, string>}
 */
export function productionCsp(confPath) {
  /** @type {unknown} */
  const parsed = JSON.parse(readFileSync(confPath, "utf8"));
  const conf = /** @type {{ app: { security: { csp: Record<string, string> } } }} */ (parsed);
  return conf.app.security.csp;
}

/** @param {Record<string, string>} directives */
export function headerValue(directives) {
  return Object.entries(directives)
    .map(([directive, sources]) => `${directive} ${sources}`)
    .join("; ");
}

/**
 * What Tauri does to each page it serves: every `<style>` element without a nonce gets
 * a fresh one, which is added to `style-src`.
 * @param {string} html @param {Record<string, string>} directives @param {string} nonce
 */
export function withStyleNonce(html, directives, nonce) {
  const marked = html.replace(/<style(?![^>]*\bnonce=)/g, `<style nonce="${nonce}"`);
  if (marked === html) return { html, directives };
  const styleSrc = directives["style-src"] ?? "'self'";
  return {
    html: marked,
    directives: { ...directives, "style-src": `${styleSrc} 'nonce-${nonce}'` },
  };
}

/**
 * The file to serve for a request path: a file inside `root`, or the app's
 * `index.html` for client-side routes. `null` for paths escaping `root`.
 * @param {string} root @param {string} urlPath
 */
export function fileFor(root, urlPath) {
  const relative = extname(urlPath) === "" ? "index.html" : decodeURIComponent(urlPath);
  const file = normalize(join(root, relative));
  return file.startsWith(root + sep) ? file : null;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [dist, port] = process.argv.slice(2);
  if (dist === undefined || port === undefined) {
    console.error("usage: serve-with-csp.js <dist directory> <port>");
    process.exit(2);
  }
  const root = resolve(dist);
  const csp = productionCsp(
    fileURLToPath(new URL("../../src-tauri/tauri.conf.json", import.meta.url)),
  );
  createServer((req, res) => {
    const file = fileFor(root, new URL(req.url ?? "/", "http://localhost").pathname);
    if (file === null) {
      res.writeHead(403).end();
      return;
    }
    readFile(file).then(
      (body) => {
        const page = extname(file) === ".html";
        const served = page
          ? withStyleNonce(body.toString("utf8"), csp, randomBytes(12).toString("base64url"))
          : { html: null, directives: csp };
        res.writeHead(200, {
          "content-type": TYPES[extname(file)] ?? "application/octet-stream",
          "content-security-policy": headerValue(served.directives),
        });
        res.end(served.html ?? body);
      },
      () => res.writeHead(404).end(),
    );
  }).listen(Number(port), "127.0.0.1", () => {
    console.log(`Serving ${root} with the production CSP on http://127.0.0.1:${port}`);
  });
}
