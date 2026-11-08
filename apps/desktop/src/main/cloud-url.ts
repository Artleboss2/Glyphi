declare const GLYPHI_CLOUD_URL: string;

/** The hosted cloud origin, or a local one for development builds. Until Glyphi runs
 * its own account service, saved usage and public profiles use the live UsageAtlas
 * deployment. */
export const CLOUD_URL =
  typeof GLYPHI_CLOUD_URL === "string"
    ? GLYPHI_CLOUD_URL
    : "https://usageatlas.com";
