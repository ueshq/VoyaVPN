import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    // Transport-agnostic logic: stores, key maps and code→text tables. Nothing
    // here touches the DOM, and the storage seam is injected per test.
    environment: "node",
  },
});
