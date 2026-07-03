import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and does not clean up on failure.
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  // Prevent Vite from obscuring Rust errors.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Don't watch the Rust backend or the archived Electron sources.
      ignored: ["**/src-tauri/**", "**/legacy-electron/**"],
    },
  },

  // Produce assets Tauri can serve as a static frontend.
  build: {
    target: "esnext",
    minify: "esbuild",
    sourcemap: false,
  },
});
