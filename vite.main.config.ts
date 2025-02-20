import { defineConfig } from "vite";
import path from "path";

// https://vitejs.dev/config
export default defineConfig({
  build: {
    rollupOptions: {
      external: [
        "better-sqlite3",
        "electron",
        "@grpc/grpc-js",
        "electron-squirrel-startup",
        "electron-log/main",
        "@bufbuild/protobuf/wire",
      ],
    },
    outDir: ".vite/build",
  },
  plugins: [
    {
      name: "configure-better-sqlite3",
      config: () => ({
        optimizeDeps: {
          exclude: ["better-sqlite3"],
        },
      }),
    },
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
