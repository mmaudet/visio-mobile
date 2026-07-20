import type { CSSProperties, ReactNode } from 'react'

export interface DeskWindowProps {
  children: ReactNode
  screenBg?: string
  /** Optional inner background (e.g. radial gradient for the Call screen). */
  style?: CSSProperties
}

/**
 * Plein-écran : la fenêtre Tauri elle-même fournit le chrome (border, ombre,
 * traffic lights gérés par l'OS). On ne reproduit donc pas la "fausse fenêtre"
 * 1200×750 du design ici — la maquette était un cadre, l'app réelle remplit
 * sa fenêtre native.
 */
export function DeskWindow({
  children,
  screenBg,
  style,
}: Readonly<DeskWindowProps>) {
  return (
    <div
      style={{
        width: '100%',
        height: '100vh',
        background: screenBg || 'var(--bg)',
        color: 'var(--text)',
        fontFamily: 'var(--font-ui)',
        display: 'flex',
        overflow: 'hidden',
        position: 'relative',
        ...style,
      }}
    >
      {children}
    </div>
  )
}

export default DeskWindow
