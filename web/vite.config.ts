import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: { proxy: { "/api": "http://127.0.0.1:4173" } },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          markdown: ["react-markdown", "remark-gfm"],
          primitives: ["@radix-ui/react-dialog", "@radix-ui/react-tabs"],
        },
      },
    },
  },
});
