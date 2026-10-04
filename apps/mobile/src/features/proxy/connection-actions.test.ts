import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { closeSingleConnection } from "./connection-actions";

test("a missing row id can never become a close-all command", async () => {
  registerMobileBackend(mockTransport());
  await expect(closeSingleConnection(null)).rejects.toThrow();
  await expect(closeSingleConnection("")).rejects.toThrow();
  expect(mockBackend().state.calls.some((call) => call.command === "proxyCloseConnection")).toBe(false);
});
