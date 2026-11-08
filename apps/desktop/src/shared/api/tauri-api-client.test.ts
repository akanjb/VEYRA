import { describe, expect, it, vi } from "vitest";

import { ApiCallError, TransportError } from "./api-errors";
import { createTauriApiClient, type InvokeFn } from "./tauri-api-client";

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
});
