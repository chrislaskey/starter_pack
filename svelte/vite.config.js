import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// The theme script sets `data-theme` on <html> before the first paint. The
// `<!-- theme-script -->` marker in an HTML entry becomes an inline classic
// script with the file's contents, in dev and in builds.
const THEME_SCRIPT = resolve(import.meta.dirname, "src/lib/theme_script.js");
const THEME_SCRIPT_MARKER = "<!-- theme-script -->";

function themeScript() {
  return {
    name: "theme-script",
    transformIndexHtml(html) {
      return html.replace(THEME_SCRIPT_MARKER, () => `<script>\n${readFileSync(THEME_SCRIPT, "utf8")}</script>`);
    },
    configureServer(server) {
      server.watcher.add(THEME_SCRIPT);
      server.watcher.on("change", (file) => {
        if (file === THEME_SCRIPT) server.ws.send({ type: "full-reload" });
      });
    },
  };
}

// Where `pnpm dev` sends `/api` requests: the web server (../axum).
// Inside the Tauri app the pages call commands instead, see src/lib/core/.
const WEB_SERVER_URL = process.env.WEB_SERVER_URL ?? "http://localhost:4000";

// `cargo tauri ios dev` and `android dev` set TAURI_DEV_HOST to this Mac's
// address on the network, so a phone or simulator reaches the dev server.
const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;

// One HTML entry per window: src/index.html, whose router
// (src/lib/router.js) shows the page at the address, and
// src/quick-picker.html for the desktop app's quick picker. The dev server
// answers any other path with index.html, and so do the web server
// (../axum) and the Tauri app, so every page is reloadable. `pnpm build` writes dist/, which the web server
// serves and the Tauri app bundles.
export default defineConfig({
  root: "src",
  plugins: [themeScript(), tailwindcss(), svelte()],
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
    rolldownOptions: {
      input: {
        main: resolve(import.meta.dirname, "src/index.html"),
        quick_picker: resolve(import.meta.dirname, "src/quick-picker.html"),
      },
    },
  },
});
