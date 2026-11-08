import { useEffect, useState } from "react";

/**
 * The opening beat: the Glyphi mark blinks behind a glass lid, glances right then
 * left, turns red and sweeps the screen with a laser before handing over to the
 * app. It runs once per launch (3.1s), can be dismissed with a click or Escape,
 * and is skipped entirely when the system asks for reduced motion.
 *
 * The mark itself is the shipped icon — the same artwork as the installer and the
 * taskbar — so the launch screen never drifts from the brand. The lid, the red
 * pass and the laser are overlays on top of it.
 */
const DURATION_MS = 3100;

let played = false;

export function BootEye(): React.JSX.Element | null {
  const [visible, setVisible] = useState(() => !played && !prefersReducedMotion());
  const [leaving, setLeaving] = useState(false);

  useEffect(() => {
    if (!visible) return;
    played = true;
    document.documentElement.dataset.boot = "eye";
    const timer = setTimeout(() => setLeaving(true), DURATION_MS - 320);
    const remove = setTimeout(() => {
      setVisible(false);
      delete document.documentElement.dataset.boot;
    }, DURATION_MS);
    const dismiss = () => setLeaving(true);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") dismiss();
    };
    window.addEventListener("pointerdown", dismiss);
    window.addEventListener("keydown", onKey);
    return () => {
      clearTimeout(timer);
      clearTimeout(remove);
      window.removeEventListener("pointerdown", dismiss);
      window.removeEventListener("keydown", onKey);
      delete document.documentElement.dataset.boot;
    };
  }, [visible]);

  if (!visible) return null;

  return (
    <div className="atlas-boot" data-leaving={leaving ? "true" : "false"} role="presentation">
      <div className="atlas-boot__glow" aria-hidden="true" />
      <div className="atlas-boot__haze" aria-hidden="true" />
      <div className="atlas-boot__mark">
        <img className="atlas-boot__glyph" src="./glyphi.png" alt="" draggable={false} />
        <span className="atlas-boot__sheen" aria-hidden="true" />
        <span className="atlas-boot__lid" aria-hidden="true" />
        <span className="atlas-boot__red" aria-hidden="true" />
      </div>
      <p className="atlas-boot__word" aria-hidden="true">Glyphi</p>
      <span className="atlas-boot__laser" aria-hidden="true" />
      <span className="sr-only">Opening Glyphi</span>
    </div>
  );
}

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
    : false;
}
