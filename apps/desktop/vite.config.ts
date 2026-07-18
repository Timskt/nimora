import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    manifest: true,
    chunkSizeWarningLimit: 650,
  },
  server: { strictPort: true, port: 1420 },
});
