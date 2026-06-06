/**
 * Marque officielle de l'app : anneau ouvert (arc Bleu France) + point Rouge
 * Marianne. En thème sombre, l'arc passe au Bleu France clair (#8C9CFF) pour
 * conserver le contraste — règle figée par le handoff.
 */
export interface VisioMarkProps {
  size?: number
  dark?: boolean
  className?: string
}

export function VisioMark({
  size = 26,
  dark = false,
  className,
}: VisioMarkProps) {
  const arc = dark ? '#8C9CFF' : '#000091'
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      style={{ display: 'block', flexShrink: 0 }}
      className={className}
      aria-hidden
    >
      <g transform="rotate(-58 24 24)">
        <circle
          cx="24"
          cy="24"
          r="16"
          stroke={arc}
          strokeWidth="5"
          strokeLinecap="round"
          strokeDasharray="70 30.5"
        />
      </g>
      <circle cx="24" cy="24" r="6.5" fill="#E1000F" />
    </svg>
  )
}

export default VisioMark
