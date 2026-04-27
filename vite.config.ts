import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          const normalizedId = id.replaceAll("\\", "/");
          if (!normalizedId.includes("/node_modules/")) {
            return undefined;
          }
          if (
            normalizedId.includes("/node_modules/react/") ||
            normalizedId.includes("/node_modules/react-dom/") ||
            normalizedId.includes("/node_modules/scheduler/")
          ) {
            return "vendor-react";
          }
          if (normalizedId.includes("/node_modules/@xyflow/")) {
            return "vendor-react-flow";
          }
          if (
            normalizedId.includes("/node_modules/react-markdown/") ||
            normalizedId.includes("/node_modules/remark-") ||
            normalizedId.includes("/node_modules/micromark") ||
            normalizedId.includes("/node_modules/mdast") ||
            normalizedId.includes("/node_modules/hast") ||
            normalizedId.includes("/node_modules/unified")
          ) {
            return "vendor-markdown";
          }
          if (normalizedId.includes("/node_modules/@tauri-apps/")) {
            return "vendor-tauri";
          }
          if (normalizedId.includes("/node_modules/lucide-react/")) {
            return "vendor-icons";
          }
          if (
            normalizedId.includes("/node_modules/zustand/") ||
            normalizedId.includes("/node_modules/i18next") ||
            normalizedId.includes("/node_modules/react-i18next/")
          ) {
            return "vendor-state";
          }
          return undefined;
        },
      },
    },
  },
}));
