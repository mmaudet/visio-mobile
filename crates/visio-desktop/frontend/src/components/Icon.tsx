import type { CSSProperties } from 'react'

// Port TS de design_handoff_desktop_redesign/design/app/kit-icons.jsx.
// Trait = currentColor ; toutes les icônes en 24×24, strokeLinecap/Join round.
const ICON_PATHS: Record<string, string> = {
  mic: '<rect x="9" y="2.5" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M8.5 21h7"/>',
  micOff:
    '<path d="M9 9v1.5a3 3 0 0 0 4.5 2.6M15 12.3V5.5a3 3 0 0 0-5.6-1.5M5.5 11a6.5 6.5 0 0 0 9.8 5.6M12 17.5V21M8.5 21h7M3.5 3.5l17 17"/>',
  video:
    '<rect x="2.5" y="6" width="13" height="12" rx="2.5"/><path d="M15.5 10.5l5-3.2v9.4l-5-3.2"/>',
  videoOff:
    '<path d="M15.5 10.5l5-3.2v9.4l-5-3.2M15.5 9.2V8.5A2.5 2.5 0 0 0 13 6H7M3 6.4A2.5 2.5 0 0 0 2.5 8.5v7A2.5 2.5 0 0 0 5 18h8.5M3.5 3.5l17 17"/>',
  phone:
    '<path d="M6.5 4h-2A1.5 1.5 0 0 0 3 5.6C3.3 13 9 18.7 16.4 19a1.5 1.5 0 0 0 1.6-1.5v-2a1.5 1.5 0 0 0-1.2-1.4l-2.3-.5a1.5 1.5 0 0 0-1.5.6l-.5.7a11 11 0 0 1-4.7-4.7l.7-.5a1.5 1.5 0 0 0 .6-1.5l-.5-2.3A1.5 1.5 0 0 0 6.5 4Z"/>',
  hangup:
    '<path d="M3.2 14.3c5-4.4 12.6-4.4 17.6 0M3.2 14.3l-.2-2.6M3.2 14.3l2.6.6M20.8 14.3l.2-2.6M20.8 14.3l-2.6.6"/>',
  chat: '<path d="M4 5.5h16a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 20 16.5H9l-4 3.5v-3.5h-.9A1.5 1.5 0 0 1 2.5 15V7A1.5 1.5 0 0 1 4 5.5Z"/>',
  users:
    '<path d="M8.5 11.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7ZM2.5 19.5a6 6 0 0 1 12 0M16 5a3.5 3.5 0 0 1 0 7M17.5 19.5a6 6 0 0 0-3-5.2"/>',
  user: '<path d="M12 11.5a4 4 0 1 0 0-8 4 4 0 0 0 0 8ZM4.5 20a7.5 7.5 0 0 1 15 0"/>',
  hand: '<path d="M8 11V4.8a1.6 1.6 0 0 1 3.2 0V10m0-.5V3.6a1.6 1.6 0 0 1 3.2 0V11m0-.8a1.6 1.6 0 0 1 3.2 0v4.3a6.5 6.5 0 0 1-6.5 6.5h-.7a6.3 6.3 0 0 1-4.5-1.9l-3.1-3.2a1.6 1.6 0 0 1 2.3-2.2L8 16"/>',
  sparkle:
    '<path d="M12 3l1.8 4.9L18.7 9.7 13.8 11.5 12 16.4 10.2 11.5 5.3 9.7 10.2 7.9 12 3ZM19 15l.7 1.9 1.9.7-1.9.7-.7 1.9-.7-1.9-1.9-.7 1.9-.7L19 15Z"/>',
  settings:
    '<path d="M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4Z"/><path d="M19.4 14.6a1.4 1.4 0 0 0 .3 1.5l.1.1a1.7 1.7 0 1 1-2.4 2.4l-.1-.1a1.4 1.4 0 0 0-2.4 1v.2a1.7 1.7 0 1 1-3.4 0v-.1a1.4 1.4 0 0 0-2.4-1l-.1.1a1.7 1.7 0 1 1-2.4-2.4l.1-.1a1.4 1.4 0 0 0-1-2.4h-.2a1.7 1.7 0 1 1 0-3.4h.1a1.4 1.4 0 0 0 1-2.4l-.1-.1A1.7 1.7 0 1 1 8.4 5.4l.1.1a1.4 1.4 0 0 0 1.5.3h.1a1.4 1.4 0 0 0 .9-1.3v-.2a1.7 1.7 0 1 1 3.4 0v.1a1.4 1.4 0 0 0 2.4 1l.1-.1a1.7 1.7 0 1 1 2.4 2.4l-.1.1a1.4 1.4 0 0 0-.3 1.5v.1a1.4 1.4 0 0 0 1.3.9h.2a1.7 1.7 0 1 1 0 3.4h-.1a1.4 1.4 0 0 0-1.3.9Z"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  link: '<path d="M9.5 14.5l5-5M8 11l-2 2a3.5 3.5 0 0 0 5 5l2-2M16 13l2-2a3.5 3.5 0 0 0-5-5l-2 2"/>',
  lock: '<rect x="4.5" y="10.5" width="15" height="10" rx="2.5"/><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"/>',
  shield:
    '<path d="M12 3l7 2.5v5.5c0 4.6-3 8.3-7 9.5-4-1.2-7-4.9-7-9.5V5.5L12 3Z"/><path d="M9 12l2 2 4-4"/>',
  globe:
    '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.4 3.9 5.7 4 9-.1 3.3-1.5 6.6-4 9-2.5-2.4-3.9-5.7-4-9 .1-3.3 1.5-6.6 4-9Z"/>',
  chevronRight: '<path d="M9 5l7 7-7 7"/>',
  chevronDown: '<path d="M5 9l7 7 7-7"/>',
  chevronLeft: '<path d="M15 5l-7 7 7 7"/>',
  chevronUp: '<path d="M5 15l7-7 7 7"/>',
  x: '<path d="M6 6l12 12M18 6L6 18"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 6.5"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>',
  calendar:
    '<rect x="3.5" y="5" width="17" height="16" rx="2.5"/><path d="M3.5 9.5h17M8 3v4M16 3v4"/>',
  more: '<circle cx="5" cy="12" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="19" cy="12" r="1.6"/>',
  share:
    '<path d="M12 15V4m0 0L8 8m4-4l4 4M5 13v5.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V13"/>',
  grid: '<rect x="3.5" y="3.5" width="7" height="7" rx="1.5"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.5"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.5"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.5"/>',
  copy: '<rect x="8.5" y="8.5" width="12" height="12" rx="2.5"/><path d="M15.5 8.5V6A2.5 2.5 0 0 0 13 3.5H6A2.5 2.5 0 0 0 3.5 6v7A2.5 2.5 0 0 0 6 15.5h2.5"/>',
  arrowRight: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  arrowLeft: '<path d="M19 12H5M11 18l-6-6 6-6"/>',
  smiley:
    '<circle cx="12" cy="12" r="9"/><path d="M8.5 14.5a4.5 4.5 0 0 0 7 0"/><circle cx="9" cy="9.5" r="1.1" fill="currentColor" stroke="none"/><circle cx="15" cy="9.5" r="1.1" fill="currentColor" stroke="none"/>',
  screenShare:
    '<rect x="2.5" y="4.5" width="19" height="13" rx="2.5"/><path d="M8 21h8M12 17.5V21M9.5 11l2.5-2.5L14.5 11M12 8.5V14"/>',
  signal: '<path d="M4 18v-3M9 18v-6M14 18v-9M19 18V6" stroke-width="2.4"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2.5M12 19.5V22M4.2 4.2l1.8 1.8M18 18l1.8 1.8M2 12h2.5M19.5 12H22M4.2 19.8L6 18M18 6l1.8-1.8"/>',
  moon: '<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z"/>',
  camFlip:
    '<path d="M4.5 8.5A2.5 2.5 0 0 1 7 6h1l1.2-1.8h5.6L16 6h1a2.5 2.5 0 0 1 2.5 2.5"/><path d="M19.5 12.5v3A2.5 2.5 0 0 1 17 18H7a2.5 2.5 0 0 1-2.5-2.5M14 12a2 2 0 1 1-4 0 2 2 0 0 1 4 0M17 9l2.5 1.5L17 12M7 9l-2.5 1.5L7 12"/>',
  bell: '<path d="M18 8.5a6 6 0 0 0-12 0c0 6-2.5 7.5-2.5 7.5h17S18 14.5 18 8.5ZM10 19.5a2 2 0 0 0 4 0"/>',
  logout:
    '<path d="M9 21H5.5A1.5 1.5 0 0 1 4 19.5v-15A1.5 1.5 0 0 1 5.5 3H9M15 16l4-4-4-4M19 12H9"/>',
  pin: '<path d="M12 21s7-5.6 7-11a7 7 0 1 0-14 0c0 5.4 7 11 7 11Z"/><circle cx="12" cy="10" r="2.6"/>',
  dot: '<circle cx="12" cy="12" r="5" fill="currentColor" stroke="none"/>',
  star: '<path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8L3.5 9.7l5.9-.9L12 3.5Z"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.5 2"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.6v.2"/>',
  qr: '<rect x="3.5" y="3.5" width="6.5" height="6.5" rx="1.2"/><rect x="14" y="3.5" width="6.5" height="6.5" rx="1.2"/><rect x="3.5" y="14" width="6.5" height="6.5" rx="1.2"/><path d="M14 14h3v3M20.5 14v6.5M14 20.5h3"/>',
  bluetooth: '<path d="M7 7l10 10-5 4V3l5 4L7 17"/>',
  speaker:
    '<rect x="5" y="3.5" width="14" height="17" rx="2.5"/><circle cx="12" cy="15" r="3.5"/><circle cx="12" cy="7.5" r="1.1" fill="currentColor" stroke="none"/>',
  send: '<path d="M3 12l18-9-7 18-2.5-7.5L3 12Z"/>',
}

export type IconName = keyof typeof ICON_PATHS

export interface IconProps {
  name: string
  size?: number
  stroke?: number
  style?: CSSProperties
  color?: string
  className?: string
  title?: string
}

export function Icon({
  name,
  size = 22,
  stroke = 1.8,
  style,
  color,
  className,
  title,
}: IconProps) {
  const d = ICON_PATHS[name] ?? ''
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color || 'currentColor'}
      strokeWidth={stroke}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ display: 'block', flexShrink: 0, ...style }}
      className={className}
      role={title ? 'img' : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
      dangerouslySetInnerHTML={{ __html: d }}
    />
  )
}

export default Icon
