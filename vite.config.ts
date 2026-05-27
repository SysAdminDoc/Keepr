import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // Explicit per audit (v0.22.10): never ship source maps in release
    // bundles. Vite defaults to `false`, but pinning it here makes the
    // intent auditable and protects against a future contributor
    // flipping it on for debugging and leaving it on.
    sourcemap: false,
  },
}));
