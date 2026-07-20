import type { ReactNode } from 'react'
import { Icon, type IconName } from '../Icon'

export interface RowProps {
  icon?: string
  iconBg?: string
  title: ReactNode
  sub?: ReactNode
  trailing?: ReactNode
  onAccent?: boolean
  last?: boolean
  onClick?: () => void
  ariaLabel?: string
  testId?: string
}

export function Row({
  icon,
  iconBg,
  title,
  sub,
  trailing,
  onAccent,
  last,
  onClick,
  ariaLabel,
  testId,
}: RowProps) {
  const interactive = !!onClick
  return (
    <div
      data-testid={testId}
      role={interactive ? 'button' : undefined}
      tabIndex={interactive ? 0 : undefined}
      aria-label={ariaLabel}
      onClick={onClick}
      onKeyDown={
        interactive
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                onClick?.()
              }
            }
          : undefined
      }
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 14,
        padding: 'var(--row-pad) 0',
        borderBottom: last ? 'none' : '1px solid var(--hair)',
        cursor: interactive ? 'pointer' : 'default',
      }}
    >
      {icon && (
        <div
          style={{
            width: 36,
            height: 36,
            borderRadius: 10,
            flexShrink: 0,
            background: iconBg || 'var(--surface-2)',
            color: onAccent ? 'var(--accent)' : 'var(--text-2)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <Icon name={icon as IconName} size={19} />
        </div>
      )}
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontSize: 15,
            fontWeight: 600,
            color: 'var(--text)',
            letterSpacing: '-0.01em',
          }}
        >
          {title}
        </div>
        {sub && (
          <div
            style={{
              fontSize: 12.5,
              color: 'var(--text-3)',
              marginTop: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {sub}
          </div>
        )}
      </div>
      {trailing}
    </div>
  )
}

export default Row
