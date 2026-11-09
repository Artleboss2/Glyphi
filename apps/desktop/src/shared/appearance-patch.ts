import {
  isAccentTint,
  isAppMark,
  isAppearanceDensity,
  isAppearanceFont,
  isAppearanceSurface,
  isAppearanceTheme,
  isNumberStyle
} from "./desktop-api";

/**
 * Validates an appearance patch from the renderer before it is sanitised and
 * stored. Every field the settings page can change has to be listed here: a
 * field missing from this allowlist silently rejects the whole update, which
 * is how the theme selector used to apply its colours and then lose them.
 */
export function isAppearancePatch(value: unknown): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const patch = value as Record<string, unknown>;
  if (Object.keys(patch).some((key) => ![
    "theme", "font", "fontScale", "textColor", "mark", "numberStyle", "accent",
    "surface", "animate", "density", "windowOpacity", "blur", "providerColors"
  ].includes(key))) return false;
  return (patch.theme === undefined || isAppearanceTheme(patch.theme))
    && (patch.font === undefined || isAppearanceFont(patch.font))
    && (patch.mark === undefined || isAppMark(patch.mark))
    && (patch.numberStyle === undefined || isNumberStyle(patch.numberStyle))
    && (patch.accent === undefined || isAccentTint(patch.accent))
    && (patch.surface === undefined || isAppearanceSurface(patch.surface))
    && (patch.density === undefined || isAppearanceDensity(patch.density))
    && (patch.animate === undefined || typeof patch.animate === "boolean")
    && (patch.fontScale === undefined || (
      typeof patch.fontScale === "number"
      && Number.isFinite(patch.fontScale)
      && patch.fontScale >= 0.85
      && patch.fontScale <= 1.3
    ))
    && (patch.windowOpacity === undefined || (typeof patch.windowOpacity === "number" && patch.windowOpacity >= 0.7 && patch.windowOpacity <= 1))
    && (patch.blur === undefined || (typeof patch.blur === "number" && Number.isInteger(patch.blur) && patch.blur >= 0 && patch.blur <= 32))
    && (patch.providerColors === undefined || (Boolean(patch.providerColors) && typeof patch.providerColors === "object" && !Array.isArray(patch.providerColors)))
    && (patch.textColor === undefined || patch.textColor === null
      || (typeof patch.textColor === "string" && /^#[0-9a-f]{6}$/iu.test(patch.textColor)));
}
