import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// GitHub Pages project site: https://starcloud530.github.io/pixelforge-ai/
export default defineConfig({
  base: "/pixelforge-ai/",
  plugins: [react(), tailwindcss()],
  server: {
    host: "0.0.0.0",
    port: 5174,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
