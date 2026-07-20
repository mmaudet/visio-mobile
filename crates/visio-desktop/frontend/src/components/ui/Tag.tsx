import type { ReactNode } from 'react'

type Tone = 'live' | 'accent' | 'danger' | 'warn' | 'neutral'

const TONES: Record<Tone, [string, string]> = {
  live: ['var(--live)', 'color-mix(in oklab, var(--live) 15%, var(--surface))'],
  accent: ['var(--accent)', 'var(--accent-soft)'],
  danger: [
    'var(--danger)',
    'color-mix(in oklab, var(--danger) 15%, var(--surface))',
  ],
  warn: ['var(--warn)', 'color-mix(in oklab, var(--warn) 16%, var(--surface))'],
  neutral: ['var(--text-2)', 'var(--surface-2)'],
}

export interface TagProps {
  tone?: Tone
  dot?: boolean
  children: ReactNode
}

export function Tag({ tone = 'neutral', dot, children }: Readonly<TagProps>) {
  const [fg, bg] = TONES[tone]
  return (
    <span className="v-tag" style={{ color: fg, background: bg }}>
      {dot && (
        <span
          style={{
            width: 6,
            height: 6,
            borderRadius: '50%',
            background: fg,
            display: 'inline-block',
          }}
        />
      )}
      {children}
    </span>
  )
}

export default Tag
