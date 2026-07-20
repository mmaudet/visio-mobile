import type { CSSProperties, ReactNode } from 'react'
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
}: Readonly<RowProps>) {
  const interactive = !!onClick
  const rowStyle: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 14,
    padding: 'var(--row-pad) 0',
    borderBottom: last ? 'none' : '1px solid var(--hair)',
  }
  const main = (
    <>
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
    </>
  )
  if (!interactive) {
    return (
      <div data-testid={testId} aria-label={ariaLabel} style={rowStyle}>
        {main}
        {trailing}
      </div>
    )
  }
  // Interactive variant: the <button> wraps the main content only and
  // `trailing` stays a sibling — interactive trailing content (e.g. the
  // calendar disconnect button) must never nest a <button> in a <button>.
  // data-testid stays on the outer row so text assertions span trailing too.
  return (
    <div data-testid={testId} style={rowStyle}>
      <button
        type="button"
        aria-label={ariaLabel}
        onClick={onClick}
        style={{
          flex: 1,
          minWidth: 0,
          display: 'flex',
          alignItems: 'center',
          gap: 14,
          padding: 0,
          background: 'none',
          border: 'none',
          cursor: 'pointer',
          font: 'inherit',
          color: 'inherit',
          textAlign: 'left',
          fontFamily: 'var(--font-ui)',
        }}
      >
        {main}
      </button>
      {trailing}
    </div>
  )
}

export default Row
