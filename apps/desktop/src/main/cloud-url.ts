declare const GLYPHI_CLOUD_URL: string;

/** The Glyphi cloud API: a Supabase Edge Function that owns the database and the
 * account sessions. `GLYPHI_CLOUD_URL` points a development build at a local copy. */
export const CLOUD_URL =
  typeof GLYPHI_CLOUD_URL === "string"
    ? GLYPHI_CLOUD_URL
    : "https://wbiwcjziutveniglptyw.supabase.co/functions/v1/glyphi-api";
