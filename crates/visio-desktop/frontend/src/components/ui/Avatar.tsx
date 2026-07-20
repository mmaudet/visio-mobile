const AV_TONES = [
  '#5b6cff',
  '#e0653a',
  '#16a07a',
  '#c0418f',
  '#7a55d6',
  '#3a7bd5',
  '#d99413',
]

function avTone(name: string) {
  let s = 0
  for (const ch of name) s += ch.codePointAt(0) ?? 0
  return AV_TONES[s % AV_TONES.length]
}

function initials(name: string) {
  return name
    .split(' ')
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0])
    .join('')
    .toUpperCase()
}

export interface AvatarProps {
  name?: string
  size?: number
  src?: string
  ring?: number
}

export function Avatar({
  name = '',
  size = 36,
  src,
  ring,
}: Readonly<AvatarProps>) {
  return (
    <div
      aria-hidden
      style={{
        width: size,
        height: size,
        borderRadius: '50%',
        flexShrink: 0,
        background: src ? `center/cover url(${src})` : avTone(name),
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        color: '#fff',
        fontWeight: 600,
        fontSize: size * 0.4,
        fontFamily: 'var(--font-ui)',
        boxShadow: ring ? `0 0 0 ${ring}px var(--ring-color)` : 'none',
        letterSpacing: '-0.02em',
        userSelect: 'none',
      }}
    >
      {!src && initials(name)}
    </div>
  )
}

export default Avatar
