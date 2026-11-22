import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// The site is served by GitHub Pages from the repo's docs/ folder, at
// cjpowis.github.io/transclip/, so the build goes there and every link
// to its own files is relative.
export default defineConfig({
  base: "./",
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    outDir: "../docs",
    emptyOutDir: true,
  },
})
