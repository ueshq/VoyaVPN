import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

// Vite only builds the stylesheet. The HTML is rendered by scripts/build.mjs,
// which reads the manifest to link the hashed file.
export default defineConfig({
  plugins: [tailwindcss()],
  build: {
    emptyOutDir: true,
    manifest: true,
    outDir: "dist",
    rollupOptions: { input: { site: "src/site.css" } },
  },
});
