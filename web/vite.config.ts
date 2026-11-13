import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: { proxy: { "/api": {
    target: "http://127.0.0.1:4173",
    changeOrigin: true,
    configure(proxy) {
      proxy.on("proxyReq", (request) => {
        request.removeHeader("origin");
        request.removeHeader("sec-fetch-site");
      });
    },
  } } },
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
