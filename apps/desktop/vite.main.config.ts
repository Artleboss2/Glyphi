import { defineConfig, loadEnv } from "vite";
import path from "node:path";

export default defineConfig(({ mode }) => {
  const environment = { ...loadEnv(mode, path.resolve(__dirname, "../../.."), ""), ...process.env };
  return {
    define: {
      GLYPHI_CLOUD_URL: JSON.stringify(environment.GLYPHI_CLOUD_URL || "https://usageatlas.com"),
      GLYPHI_POSTHOG_KEY: JSON.stringify(environment.POSTHOG_PROJECT_TOKEN ?? ""),
      GLYPHI_POSTHOG_HOST: JSON.stringify(environment.POSTHOG_HOST || "https://usageatlas.com/signals")
    },
    build: {
      rollupOptions: { external: ["electron", "node:sqlite"] }
    }
  };
});
