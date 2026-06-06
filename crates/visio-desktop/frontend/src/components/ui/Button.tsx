import type { ButtonHTMLAttributes, ReactNode } from 'react'

type Variant = 'primary' | 'secondary' | 'outline' | 'ghost' | 'danger'
type Size = 'md' | 'sm'

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant
  size?: Size
  full?: boolean
  icon?: ReactNode
  iconRight?: ReactNode
}

export function Button({
  variant = 'secondary',
  size = 'md',
  full,
  icon,
  iconRight,
  className,
  children,
  type = 'button',
  ...rest
}: ButtonProps) {
  const classes = [
    'v-btn',
    variant,
    size === 'sm' ? 'sm' : '',
    full ? 'full' : '',
    className ?? '',
  ]
    .filter(Boolean)
    .join(' ')
  return (
    <button {...rest} type={type} className={classes}>
      {icon}
      {children}
      {iconRight}
    </button>
  )
}

export default Button
