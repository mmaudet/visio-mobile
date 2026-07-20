export interface ToggleProps {
  on: boolean
  onChange?: (next: boolean) => void
  disabled?: boolean
  ariaLabel?: string
}

export function Toggle({
  on,
  onChange,
  disabled,
  ariaLabel,
}: Readonly<ToggleProps>) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => onChange?.(!on)}
      style={{
        width: 44,
        height: 26,
        borderRadius: 999,
        flexShrink: 0,
        border: 'none',
        cursor: disabled ? 'not-allowed' : 'pointer',
        padding: 0,
        background: on ? 'var(--accent)' : 'var(--surface-3)',
        position: 'relative',
        transition: 'background .15s',
        opacity: disabled ? 0.55 : 1,
      }}
    >
      <span
        aria-hidden
        style={{
          position: 'absolute',
          top: 3,
          left: on ? 21 : 3,
          width: 20,
          height: 20,
          borderRadius: '50%',
          background: '#fff',
          boxShadow: '0 1px 3px rgba(0,0,0,0.25)',
          transition: 'left .15s',
        }}
      />
    </button>
  )
}

export default Toggle
