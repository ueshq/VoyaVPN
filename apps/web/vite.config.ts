import tailwindcss from "@tailwindcss/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

// Vite only builds the stylesheet. The HTML is rendered by scripts/build.mjs,
// which reads the manifest to link the hashed file.
export default defineConfig({
  plugins: lazyPlugins(() => [tailwindcss()]),
  build: {
    emptyOutDir: true,
    manifest: true,
    outDir: "dist",
    rolldownOptions: { input: { site: "src/site.css" } },
  },
  test: {
    // Pages are rendered to strings at build time; nothing runs in a browser.
    environment: "node",
  },
});
