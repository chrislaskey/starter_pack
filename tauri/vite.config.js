import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// The Svelte components Phoenix generates from its function components
// (see ../phoenix/priv/svelte/README.md), imported as `$phoenix/...`.
const PHOENIX_WEB = resolve(import.meta.dirname, "../phoenix/lib/app/web");

// Phoenix's assets/css/theme.css loads daisyUI from its mix deps, by paths
// in the daisyUI repository. The same files are in the npm package.
const DAISYUI = dirname(fileURLToPath(import.meta.resolve("daisyui")));

// Phoenix's theme script, the one its root layout inlines. Each page's
// `<!-- phoenix-theme-script -->` marker becomes an inline classic script with
// the file's contents, so it runs before the first paint, in dev and in
// builds. A page without the marker is left alone.
const THEME_SCRIPT = resolve(import.meta.dirname, "../phoenix/assets/js/theme_script.js");
const THEME_SCRIPT_MARKER = "<!-- phoenix-theme-script -->";

function phoenixThemeScript() {
  return {
    name: "phoenix-theme-script",
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

// One HTML entry per page of the main window. `cargo tauri dev` loads the
// dev server (see `build.devUrl` in src-tauri/tauri.conf.json), and
// `cargo tauri build` bundles `dist/`.
export default defineConfig({
  root: "src",
  plugins: [phoenixThemeScript(), tailwindcss(), svelte()],
  resolve: {
    alias: [
      { find: "$phoenix", replacement: PHOENIX_WEB },
      { find: /^daisyui\/packages\/bundle\/daisyui$/, replacement: resolve(DAISYUI, "index.js") },
      { find: /^daisyui\/packages\/bundle\/daisyui-theme$/, replacement: resolve(DAISYUI, "theme/index.js") },
    ],
    // Phoenix's files have no node_modules above them, so resolve Svelte here.
    dedupe: ["svelte"],
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    fs: { allow: [import.meta.dirname, PHOENIX_WEB] },
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "src/index.html"),
        tmux_example: resolve(import.meta.dirname, "src/tmux-example.html"),
        quick_search: resolve(import.meta.dirname, "src/quick-search.html"),
      },
    },
  },
});
