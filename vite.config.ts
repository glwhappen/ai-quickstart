import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 约定：开发端口固定、产物在 dist/
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5183,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    target: "chrome105",
    minify: "esbuild",
  },
});
