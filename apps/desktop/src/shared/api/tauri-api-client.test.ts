import { describe, expect, it, vi } from "vitest";

import { ApiCallError, TransportError } from "./api-errors";
import { createTauriApiClient, type InvokeFn, type ListenFn } from "./tauri-api-client";

const appInfo = {
  productName: "Veyra",
  version: "0.1.0",
  buildProfile: "debug",
  os: "windows",
  arch: "x86_64",
} as const;

describe("createTauriApiClient", () => {
  it("sends the request under the `request` argument and returns the response", async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(appInfo);
    const client = createTauriApiClient(invoke);

    await expect(client.call("get_app_info", {})).resolves.toEqual(appInfo);
    expect(invoke).toHaveBeenCalledWith("get_app_info", { request: {} });
  });

  it("maps a typed backend error to ApiCallError", async () => {
    const error = { kind: "internal", correlationId: "c-1" };
    const client = createTauriApiClient(vi.fn<InvokeFn>().mockRejectedValue(error));

    const rejection = client.call("get_app_info", {});
    await expect(rejection).rejects.toBeInstanceOf(ApiCallError);
    await expect(rejection).rejects.toMatchObject({ command: "get_app_info", error });
  });

  it("maps any other rejection to TransportError", async () => {
    const client = createTauriApiClient(
      vi.fn<InvokeFn>().mockRejectedValue("get_app_info not allowed"),
    );

    await expect(client.call("get_app_info", {})).rejects.toEqual(
      new TransportError("get_app_info", "get_app_info not allowed"),
    );
  });

  it("delivers event payloads and stops listening on unsubscribe", async () => {
    let emit: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = vi.fn();
    const listen = vi.fn<ListenFn>((_name, handler) => {
      emit = handler;
      return Promise.resolve(unlisten);
    });
    const client = createTauriApiClient(vi.fn<InvokeFn>(), listen);
    const received: unknown[] = [];

    const stop = client.subscribe("data-changed", (payload) => received.push(payload));
    await Promise.resolve();
    emit?.({ payload: { inserted: 3, updated: 1 } });
    stop();

    expect(listen).toHaveBeenCalledWith("data-changed", expect.any(Function));
    expect(received).toEqual([{ inserted: 3, updated: 1 }]);
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("stops a subscription that ends before listening started", async () => {
    const unlisten = vi.fn();
    const client = createTauriApiClient(
      vi.fn<InvokeFn>(),
      vi.fn<ListenFn>(() => Promise.resolve(unlisten)),
    );
    client.subscribe("data-changed", () => undefined)();
    await Promise.resolve();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
