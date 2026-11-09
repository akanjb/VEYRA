/**
 * Lets styles that libraries inject at runtime (Radix's scroll lock in dialogs and
 * menus) pass the production Content Security Policy, which allows `<style>` elements
 * from 'self' only.
 *
 * Tauri gives every `<style>` element in index.html a fresh random nonce on each page
 * load and adds it to the policy. Injected styles adopt that nonce through `get-nonce`,
 * the module Radix's style injector reads it from. Outside Tauri there is no nonce and
 * nothing changes.
 */
import { setNonce } from "get-nonce";

/** Reads the page's style nonce, if any, for styles injected later. */
export function adoptStyleNonce(doc: Document = document): void {
  const nonce = doc.querySelector<HTMLStyleElement>("style[nonce]")?.nonce;
  if (nonce !== undefined && nonce !== "") setNonce(nonce);
}
