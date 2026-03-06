import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    fs: {
      // Allow serving files from the project root (for i18n folder)
      allow: [".", "../../.."],
    },
  },
  build: {
    outDir: "dist",
  },
});
