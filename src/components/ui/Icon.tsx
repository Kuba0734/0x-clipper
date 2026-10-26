import type { SVGProps } from "react";

/** Inline-SVG-Icons (24×24, Strich 1.75). Keine Icon-Bibliothek, nur die benötigten Formen. */
const paths = {
  dashboard: "M4 4h7v7H4zM13 4h7v4h-7zM13 10h7v10h-7zM4 13h7v7H4z",
  film: "M4 5h16v14H4zM8 5v14M16 5v14M4 9.5h4M4 14.5h4M16 9.5h4M16 14.5h4",
  settings:
    "M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4zM19.4 13.5l1.6 1.2-2 3.4-1.9-.7a7.6 7.6 0 0 1-2 1.2l-.3 2h-4l-.3-2a7.6 7.6 0 0 1-2-1.2l-1.9.7-2-3.4 1.6-1.2a7.7 7.7 0 0 1 0-3L2.6 9.3l2-3.4 1.9.7a7.6 7.6 0 0 1 2-1.2l.3-2h4l.3 2a7.6 7.6 0 0 1 2 1.2l1.9-.7 2 3.4-1.6 1.2a7.7 7.7 0 0 1 0 3z",
  sliders: "M4 7h10M18 7h2M4 17h4M12 17h8M14 4.5v5M8 14.5v5",
  timer: "M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM12 9v4l2.5 2M9.5 2.5h5",
  monitor: "M3 4.5h18v12H3zM8.5 20.5h7M12 16.5v4",
  monitorPlay: "M3 4.5h18v12H3zM8.5 20.5h7M12 16.5v4M10.5 8l4 2.5-4 2.5z",
  audio: "M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2",
  keyboard: "M3 6h18v12H3zM7 10h.01M11 10h.01M15 10h.01M7 14h10",
  folderOut: "M3 6.5a1.5 1.5 0 0 1 1.5-1.5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5zM12 11v6M9.5 13.5 12 11l2.5 2.5",
  layers: "M12 3 3 8l9 5 9-5zM3 12.5l9 5 9-5M3 16.5l9 5 9-5",
  folder: "M3 6.5a1.5 1.5 0 0 1 1.5-1.5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z",
  folderSearch: "M3 6.5a1.5 1.5 0 0 1 1.5-1.5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v3M10 20H4.5A1.5 1.5 0 0 1 3 18.5V9M16.5 19a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM18.5 18.5 21 21",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4",
  chevronDown: "M6 9l6 6 6-6",
  x: "M6 6l12 12M18 6 6 18",
  minus: "M5 12h14",
  square: "M5.5 5.5h13v13h-13z",
  restore: "M8 8h11v11H8zM5 16V5h11",
  check: "M5 12.5l4.5 4.5L19 7.5",
  alert: "M12 4 2.5 20h19zM12 10v4.5M12 17.5h.01",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v6M12 7.5h.01",
  play: "M8 5.5v13l10.5-6.5z",
  clapper: "M4 9.5h16V20H4zM4 9.5 6.5 4l13 1.5L18 9.5M9 4.5l-1.5 5M14 5l-1.5 4.5",
  cpu: "M7 7h10v10H7zM10 10h4v4h-4zM9.5 3v4M14.5 3v4M9.5 17v4M14.5 17v4M3 9.5h4M3 14.5h4M17 9.5h4M17 14.5h4",
  gauge: "M4.5 17a8 8 0 1 1 15 0M12 13l3.5-4.5M12 14.5a1.5 1.5 0 1 0 0-3",
  drive: "M3 13.5 5.5 5h13l2.5 8.5V19H3zM3 13.5h18M7 16.5h.01M10.5 16.5h.01",
  memory: "M4 7h16v9H4zM7 16v3M11 16v3M15 16v3M8 10.5v2M12 10.5v2M16 10.5v2",
  micOff: "M9 9.5V6a3 3 0 0 1 5.8-1.1M15 11v.5a3 3 0 0 1-4.9 2.3M18 11.5a6 6 0 0 1-9.8 4.6M6 11.5a6 6 0 0 0 .7 2.8M12 17.5V21M4 4l16 16",
  power: "M12 3v8M7 6.2a7.5 7.5 0 1 0 10 0",
  scissors: "M6.5 9a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM6.5 20a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM8.5 7.5 20 18M8.5 16.5 20 6",
  trash: "M4 7h16M9.5 7V4.5h5V7M6 7l1 13h10l1-13M10 11v5.5M14 11v5.5",
  refresh: "M20 11a8 8 0 0 0-14.5-4.5L4 8M4 4v4h4M4 13a8 8 0 0 0 14.5 4.5L20 16M20 20v-4h-4",
  reset: "M4 4v5h5M4.6 13.5A7.5 7.5 0 1 0 6 7.4L4 9",
  upload: "M12 15V4M7.5 8.5 12 4l4.5 4.5M4 15v4.5h16V15",
  download: "M12 4v11M7.5 10.5 12 15l4.5-4.5M4 15v4.5h16V15",
  record: "M12 12h.01",
} as const;

export type IconName = keyof typeof paths;

interface Props extends Omit<SVGProps<SVGSVGElement>, "name"> {
  name: IconName;
  size?: number;
}

export function Icon({ name, size = 18, className = "", ...rest }: Props) {
  if (name === "record") {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden className={className} {...rest}>
        <circle cx="12" cy="12" r="6" fill="currentColor" />
      </svg>
    );
  }
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      className={`shrink-0 ${className}`}
      {...rest}
    >
      <path d={paths[name]} />
    </svg>
  );
}
