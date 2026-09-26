import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { mockTransport } from "~/test/mock-transport";
import type { MockBackend } from "@voya/client/mock-backend";
import { closeSingleConnection } from "./connection-actions";

test("a missing row id can never become a close-all command", async () => {
  registerMobileBackend(mockTransport());
  await expect(closeSingleConnection(null)).rejects.toThrow();
  await expect(closeSingleConnection("")).rejects.toThrow();
  expect((voyaTransport() as MockBackend).state.calls.some((call) => call.command === "proxyCloseConnection")).toBe(false);
});
