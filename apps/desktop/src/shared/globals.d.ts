import type { GlyphiDesktopAPI } from "./desktop-api";

declare global {
  interface Window {
    glyphi: GlyphiDesktopAPI;
  }
}

export {};
