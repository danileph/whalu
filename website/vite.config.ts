import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  // Relative asset URLs also work beneath a GitHub Pages repository path.
  base: "./",
  server: { port: 5173, strictPort: true },
});
