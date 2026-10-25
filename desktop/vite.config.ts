import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const page = (name: string) => fileURLToPath(new URL(`${name}.html`, import.meta.url));

// Three pages, one per Tauri window.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    target: ["es2022", "chrome105", "safari15"],
    rollupOptions: {
      input: {
        floating: page("floating"),
        fullscreen: page("fullscreen"),
        settings: page("settings"),
      },
    },
  },
});
