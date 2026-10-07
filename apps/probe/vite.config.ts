import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    // The handler is plain fetch-API code with the socket dialer injected, so
    // it runs under Node without the Workers runtime.
    environment: "node",
  },
});
