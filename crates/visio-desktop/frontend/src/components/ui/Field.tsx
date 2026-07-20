import type { InputHTMLAttributes, ReactNode } from 'react'
import { Icon, type IconName } from '../Icon'

export interface FieldProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'size'
> {
  icon?: string
  trailing?: ReactNode
  mono?: boolean
  big?: boolean
  /** When true, the field is read-only and clickable (used for picker rows). */
  asButton?: boolean
}

export function Field({
  icon,
  trailing,
  mono,
  big,
  asButton,
  className,
  style,
  ...inputProps
}: Readonly<FieldProps>) {
  return (
    <div
      className={['v-input', className].filter(Boolean).join(' ')}
      style={{
        ...(big
          ? { height: 'auto', minHeight: 'var(--ctl-h)', padding: '12px 14px' }
          : null),
        ...style,
      }}
    >
      {icon && (
        <Icon
          name={icon as IconName}
          size={18}
          style={{ color: 'var(--text-3)' }}
        />
      )}
      <input
        {...inputProps}
        className={mono ? 'v-mono' : undefined}
        style={{
          letterSpacing: mono ? '0.06em' : undefined,
          fontWeight: mono ? 600 : 500,
          fontSize: mono ? 15 : 14.5,
          cursor: asButton ? 'pointer' : undefined,
        }}
        readOnly={asButton || inputProps.readOnly}
      />
      {trailing}
    </div>
  )
}

export default Field
