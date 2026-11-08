import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

/**
 * The interface glyphs are drawn as a small Egyptian vocabulary: sun discs,
 * winged horizons, cartouches, stelae, obelisks, ankhs, scarabs and the scales
 * of Maat. Every icon keeps the same 24 px grid and stroke weight as the rest of
 * the shell, so the set reads as one hand rather than a theme applied on top.
 */
function IconBase({ children, ...props }: IconProps): React.JSX.Element {
  return (
    <svg
      aria-hidden="true"
      fill="none"
      height="20"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth="1.8"
      viewBox="0 0 24 24"
      width="20"
      {...props}
    >
      {children}
    </svg>
  );
}

/** Today: the sun rising over the Nile, with two wing strokes. */
export function DayIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="11" r="3.4" />
      <path d="M12 2.8v2M3.2 11h2M18.8 11h2M5.1 4.9l1.5 1.5M17.4 6.4l1.5-1.5" />
      <path d="M3.5 16.5h17" />
      <path d="M3.5 20c2.8-1.7 5.7-1.7 8.5 0s5.7 1.7 8.5 0" />
    </IconBase>
  );
}

/** History: a cartouche holding a sun disc, wound round by a serpent. */
export function HistoryIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <rect height="17" rx="8.5" width="12.5" x="6" y="3.5" />
      <circle cx="12" cy="9.2" r="2.1" />
      <path d="M12 11.3v4.4l2.2 1.5" />
      <path d="M4.5 15.5c-1.6.4-2.4 1.4-2.2 2.6.2 1.1 1.3 1.8 2.6 1.6" />
    </IconBase>
  );
}

/** A range of days: a round-topped stela with two registers. */
export function CalendarRangeIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M7 21V7a5 5 0 0 1 10 0v14" />
      <path d="M4.5 21h15M9.5 10.5h5M9.5 14.5h5M9.5 18.5h3" />
    </IconBase>
  );
}

/** Trends: a stair of sandstone blocks with a rising sun. */
export function TrendsIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M3 20h4v-4h4v-4h4V8h5" />
      <path d="M16.5 4.6 20 8l-3.5 3.4" />
      <circle cx="5" cy="17" r="0.4" />
    </IconBase>
  );
}

/** Insights: three obelisks at different heights. */
export function InsightsIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M4 20V10l1.7-2.6L7.4 10v10M10.2 20V5.4L12 2.8l1.8 2.6V20M16.6 20V12l1.7-2.6L20 12v8" />
      <path d="M2.5 20h19" />
    </IconBase>
  );
}

/** Providers: three canopic jars, the sources the app reads from. */
export function ProvidersIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M5 9.5h3.4V20H5zM5 8.2a1.7 1.7 0 0 1 3.4 0" />
      <path d="M10.3 9.5h3.4V20h-3.4zM10.3 8.2a1.7 1.7 0 0 1 3.4 0" />
      <path d="M15.6 9.5H19V20h-3.4zM15.6 8.2a1.7 1.7 0 0 1 3.4 0" />
      <path d="M3.5 20h17" />
    </IconBase>
  );
}

export function ChevronLeftIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="m15 18-6-6 6-6" />
      <path d="M6.5 12h.01" />
    </IconBase>
  );
}

export function ChevronRightIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="m9 18 6-6-6-6" />
      <path d="M17.5 12h.01" />
    </IconBase>
  );
}

export function ChevronDownIcon(props: IconProps): React.JSX.Element {
  return <IconBase {...props}><path d="m6 9 6 6 6-6" /></IconBase>;
}

/** Settings: a sun disc ringed by rays, with the cobra at its brow. */
export function SettingsIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="11" r="3.2" />
      <path d="M12 2.5v2.2M12 17.3v2.2M2.5 11h2.2M19.3 11h2.2M5.3 4.3l1.6 1.6M17.1 16.1l1.6 1.6M18.7 4.3l-1.6 1.6M6.9 16.1l-1.6 1.6" />
      <path d="M9.2 13.6c0 2.4 1.1 3.9 2.8 3.9s2.8-1.5 2.8-3.9" />
    </IconBase>
  );
}

/** Pulse: a beat drawn over the water of the Nile. */
export function PulseIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M2.5 12h3.2l1.8-4.4L11 16l2.1-4h3.2" />
      <path d="M2.5 19.5c2.4-1.4 4.8-1.4 7.2 0s4.8 1.4 7.2 0 4.8-1.4 4.6 0" />
    </IconBase>
  );
}

/** Refresh: the scarab, an emblem of renewal, inside a returning arrow. */
export function RefreshIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M20 8.2a8.2 8.2 0 1 0 1 5.4" />
      <path d="M20 3.4v4.8h-4.8" />
      <ellipse cx="12" cy="12.6" rx="2.4" ry="3.2" />
      <path d="M12 9.4V7.6M9.7 10.4 8.2 9M14.3 10.4 15.8 9" />
    </IconBase>
  );
}

/** Download: an offering laid on a table. */
export function DownloadIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M12 2.8v9.4m-3.4-3.4L12 12.2l3.4-3.4" />
      <path d="M4 15h16M6.5 15v6M17.5 15v6" />
    </IconBase>
  );
}

/** Night: the crescent and the stars over the desert. */
export function MoonIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M20 15.2A8.5 8.5 0 0 1 8.8 4 8.5 8.5 0 1 0 20 15.2Z" />
      <path d="M16.5 4.6v2.4M15.3 5.8h2.4M6.4 7.2v1.8M5.5 8.1h1.8" />
    </IconBase>
  );
}

/** Day: Aten, the sun disc whose rays end in hands. */
export function SunIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="12" r="3.8" />
      <path d="M12 3v3.4m0 11.2V21M3 12h3.4m11.2 0H21M5.6 5.6l2.4 2.4m8 8 2.4 2.4m0-12.8-2.4 2.4m-8 8-2.4 2.4" />
      <path d="M3 3.2v1.6M2.2 4h1.6M21 3.2v1.6M20.2 4h1.6" />
    </IconBase>
  );
}

/** Elsewhere: the false door, the threshold between rooms. */
export function ExternalIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M4 21V7.5h8.5V21M2.2 21h15.6" />
      <path d="M14.5 3.2H21v6.4M21 3.2l-6.8 6.8" />
    </IconBase>
  );
}

/** Done: a chiselled check with the sun above it. */
export function CheckIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="m4.5 12.5 4.5 4.5L19.5 6.5" />
      <circle cx="4.6" cy="4.6" r="1.1" />
    </IconBase>
  );
}

/** Close: two crossed sceptres. */
export function CloseIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M5 5l14 14M19 5 5 19" />
      <path d="M5 3.4V5M3.4 5H5M19 21v-1.6M20.6 19H19" />
    </IconBase>
  );
}

/** Warning: a pyramid with the eye of the sun inside it. */
export function AlertIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M12 3 2.8 20h18.4L12 3Z" />
      <path d="M8.4 14.2c2.3-2.4 4.9-2.4 7.2 0-2.3 2.4-4.9 2.4-7.2 0Z" />
      <path d="M12 13.2v2" />
    </IconBase>
  );
}

/** Alerts: a sistrum, the rattle that announces the goddess. */
export function BellIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M8 10a4 4 0 0 1 8 0v3" />
      <path d="M6 13h12M7.5 17h9M9 21h6" />
      <path d="M8 10v11M16 10v11" />
    </IconBase>
  );
}

/** Share: the feather of Maat lifted upwards. */
export function ShareIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M12 15V3.2m-3.2 3.4L12 3.4l3.2 3.2" />
      <path d="M12 21c-2.6-2.2-4-4.6-4-7.2 0-2.4 1.8-4.6 4-6.6 2.2 2 4 4.2 4 6.6 0 2.6-1.4 5-4 7.2Z" />
    </IconBase>
  );
}

/** Account: a pharaoh wearing the nemes headdress. */
export function ProfileIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="9.4" r="3.3" />
      <path d="M5.5 21v-2.4c0-2.1 1.6-3.6 3.7-3.6h5.6c2.1 0 3.7 1.5 3.7 3.6V21" />
      <path d="M6.5 12 5.2 6.2 8.4 5l1.4 2.1M17.5 12l1.3-5.8L15.6 5l-1.4 2.1" />
    </IconBase>
  );
}

/** Speed: lightning over an obelisk. */
export function ZapIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M12 2.5 5.5 13h5L9.8 21.5 18.5 9.5h-5.2l1.3-7z" />
    </IconBase>
  );
}

/** Burn: a brazier flame. */
export function FlameIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M12 2.8c2.1 3.2 4.2 5 4.2 7.6a4.2 4.2 0 0 1-8.4 0c0-1.6.7-2.7 1.8-3.9" />
      <path d="M4 15.5h16l-2.2 4.2H6.2z" />
    </IconBase>
  );
}

/** A day kept: a stela with the sun and a mark. */
export function CalendarCheckIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M6.5 21V6.5a5.5 5.5 0 0 1 11 0V21" />
      <path d="M4 21h16M9.4 10.6h5.2M9.4 14.8l1.9 1.9 3.4-3.4" />
    </IconBase>
  );
}

/** Launch: the winged sun rising over the horizon. */
export function RocketIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="11" r="3.2" />
      <path d="M2.5 9.2c3.4-2.4 6.2-2.4 9.5 0 3.3-2.4 6.1-2.4 9.5 0" />
      <path d="M4 15.4c2.8-1.4 5.2-1.4 8 0 2.8-1.4 5.2-1.4 8 0" />
      <path d="M12 17.5V21.4" />
    </IconBase>
  );
}

/** Tools: an adze, the carpenter's blade. */
export function WrenchIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M3 20.5 13.4 10.1" />
      <path d="M12.4 8.9c-.9-1.6-.4-3.8 1.3-4.9 1.2-.8 2.7-.8 3.9-.2l-2.7 2.7 1.8 1.8 2.7-2.7c.6 1.2.6 2.7-.2 3.9-1.1 1.7-3.3 2.2-4.9 1.3" />
      <path d="M3 20.5h.01" />
    </IconBase>
  );
}

/** Attribution: the scales of Maat. */
export function AttributionIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M12 3.6v17.6M5 21h14" />
      <path d="M4 6.6h16" />
      <circle cx="12" cy="3.4" r="1.3" />
      <path d="M4 6.6 1.4 13.2h5.2zM20 6.6l-2.6 6.6h5.2z" />
    </IconBase>
  );
}

/** Offering: a lotus vase. */
export function GiftIcon(props: IconProps): React.JSX.Element {
  return (
    <IconBase {...props}>
      <path d="M8.4 4.2c1 1.3 2.2 2 3.6 2s2.6-.7 3.6-2c.6 2.2-.4 3.8-2 4.6" />
      <path d="M7.5 9.4h9L15 21H9z" />
      <path d="M10 13.4h4" />
    </IconBase>
  );
}
