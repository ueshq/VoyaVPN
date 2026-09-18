import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // The handler is plain fetch-API code with the socket dialer injected, so
    // it runs under Node without the Workers runtime.
    environment: "node",
  },
});
