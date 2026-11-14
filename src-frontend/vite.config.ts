import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

const monacoVsPath = decodeURIComponent(
  new URL("../node_modules/monaco-editor/esm/vs", import.meta.url).pathname,
).replace(/^\/([A-Za-z]:)/, "$1");

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  resolve: {
    alias: {
      "monaco-editor/esm/vs/editor/editor.worker.js": `${monacoVsPath}/editor/editor.worker.js`,
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
  },
  envPrefix: ["VITE_", "TAURI_"],
});
