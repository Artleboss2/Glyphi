declare const GLYPHI_CLOUD_URL: string;

/** The glyphi.app origin, or a local one for development builds. */
export const CLOUD_URL =
  typeof GLYPHI_CLOUD_URL === "string"
    ? GLYPHI_CLOUD_URL
    : "https://glyphi.app";
