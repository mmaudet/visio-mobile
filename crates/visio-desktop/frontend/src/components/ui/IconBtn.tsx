import type { ButtonHTMLAttributes, CSSProperties } from 'react'
import { Icon, type IconName } from '../Icon'

type Variant =
  | 'surface'
  | 'glass'
  | 'soft'
  | 'accent'
  | 'danger'
  | 'off'
  | 'none'

export interface IconBtnProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'name'
> {
  name: string
  size?: number
  dim?: number | string
  variant?: Variant
  tint?: string
  badge?: number | string
  ariaLabel?: string
}

const BG: Record<Variant, string> = {
  surface: 'var(--surface)',
  glass: 'rgba(255,255,255,0.14)',
  soft: 'var(--surface-2)',
  accent: 'var(--accent)',
  danger: 'var(--danger)',
  off: 'rgba(255, 90, 95, 0.18)',
  none: 'transparent',
}

export function IconBtn({
  name,
  size = 20,
  dim = 'var(--ctl-h)',
  variant = 'surface',
  tint,
  badge,
  style,
  className,
  ariaLabel,
  ...rest
}: IconBtnProps) {
  const bg = BG[variant]
  const color =
    variant === 'accent' || variant === 'danger'
      ? '#fff'
      : variant === 'off'
        ? 'var(--danger)'
        : (tint ?? 'var(--text)')

  const btnStyle: CSSProperties = {
    width: dim,
    height: dim,
    borderRadius: '50%',
    border: 'none',
    cursor: 'pointer',
    background: bg,
    color,
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    backdropFilter: variant === 'glass' ? 'blur(8px)' : undefined,
    transition: 'background 0.15s, filter 0.15s',
    ...style,
  }

  return (
    <span
      style={{ position: 'relative', display: 'inline-flex' }}
      className={className}
    >
      <button type="button" {...rest} aria-label={ariaLabel} style={btnStyle}>
        <Icon name={name as IconName} size={size} />
      </button>
      {badge != null && (
        <span
          style={{
            position: 'absolute',
            top: -2,
            right: -2,
            minWidth: 18,
            height: 18,
            padding: '0 5px',
            borderRadius: 9,
            background: 'var(--accent)',
            color: '#fff',
            fontSize: 11,
            fontWeight: 700,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            boxShadow: '0 0 0 2px var(--ring-color)',
          }}
        >
          {badge}
        </span>
      )}
    </span>
  )
}

export default IconBtn
