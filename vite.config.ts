import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [react()],
  // The app kit is linked from a sibling folder with its own node_modules; make it share this app's
  // React and Lucide so hooks run against one React, and let the dev server serve its fonts.
  resolve: { dedupe: ["react", "react-dom", "react/jsx-runtime", "lucide-react"] },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
    fs: { allow: [".", "../overtone-foundry-app-kit"] },
  },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
}));
