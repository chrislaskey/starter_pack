import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Where `pnpm dev` sends `/api` requests: the web server (../axum).
// Inside the Tauri app the pages call commands instead, see src/lib/core/.
const WEB_SERVER_URL = process.env.WEB_SERVER_URL ?? "http://localhost:4000";

// `cargo tauri ios dev` and `android dev` set TAURI_DEV_HOST to this Mac's
// address on the network, so a phone or simulator reaches the dev server.
const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;

// One HTML entry, src/index.html, whose router (src/lib/router.js) shows
// the page at the address. The dev server answers any other path with
// index.html, and so do the web server (../axum) and the Tauri app, so
// every page is reloadable. `pnpm build` writes dist/, which the web server
// serves and the Tauri app bundles.
export default defineConfig({
  root: "src",
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: TAURI_DEV_HOST || false,
    hmr: TAURI_DEV_HOST ? { protocol: "ws", host: TAURI_DEV_HOST, port: 1421 } : undefined,
    proxy: { "/api": WEB_SERVER_URL },
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
});
