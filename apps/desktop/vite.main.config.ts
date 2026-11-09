import { defineConfig, loadEnv } from "vite";
import path from "node:path";

export default defineConfig(({ mode }) => {
  const environment = { ...loadEnv(mode, path.resolve(__dirname, "../../.."), ""), ...process.env };
  return {
    define: {
      GLYPHI_CLOUD_URL: JSON.stringify(
        environment.GLYPHI_CLOUD_URL || "https://wbiwcjziutveniglptyw.supabase.co/functions/v1/glyphi-api"
      ),
      GLYPHI_POSTHOG_KEY: JSON.stringify(environment.POSTHOG_PROJECT_TOKEN ?? ""),
      GLYPHI_POSTHOG_HOST: JSON.stringify(environment.POSTHOG_HOST || "https://eu.i.posthog.com")
    },
    build: {
      rollupOptions: { external: ["electron", "node:sqlite"] }
    }
  };
});
