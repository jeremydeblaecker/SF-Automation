import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],

  // Tauri expects a fixed port, fail if that port is not available
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      // Ne surveille pas le dossier de build Rust : Cargo y écrit
      // pendant la compilation, ce qui provoque des erreurs EBUSY
      // sous Windows quand Vite essaie de le "watcher" en même temps.
      ignored: ["**/src-tauri/**"],
    },
  },

  // Empêche Vite d'effacer les logs de rustc lors de l'affichage des erreurs
  clearScreen: false,

  envPrefix: ["VITE_", "TAURI_"],
});
