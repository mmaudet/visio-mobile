import { useEffect } from 'react'
import { Button } from './Button'

export interface ConfirmModalProps {
  title?: string
  message: string
  confirmLabel: string
  cancelLabel: string
  danger?: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function ConfirmModal({
  title,
  message,
  confirmLabel,
  cancelLabel,
  danger,
  onConfirm,
  onCancel,
}: Readonly<ConfirmModalProps>) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel()
      else if (e.key === 'Enter') onConfirm()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onCancel, onConfirm])
  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 10000,
      }}
    >
      {/* Backdrop: a native button so backdrop-click-to-cancel stays
          keyboard- and pointer-accessible without ARIA hacks. */}
      <button
        type="button"
        aria-label={cancelLabel}
        onClick={onCancel}
        style={{
          position: 'absolute',
          inset: 0,
          border: 'none',
          padding: 0,
          cursor: 'default',
          background: 'rgba(8,10,14,0.45)',
          backdropFilter: 'blur(2px)',
        }}
      />
      <dialog
        open
        aria-modal="true"
        style={{
          position: 'relative',
          margin: 0,
          width: 'min(420px, calc(100vw - 48px))',
          background: 'var(--surface)',
          color: 'var(--text)',
          borderRadius: 'var(--r-card)',
          boxShadow: 'var(--shadow-pop)',
          border: '1px solid var(--border)',
          padding: 22,
          display: 'flex',
          flexDirection: 'column',
          gap: 14,
        }}
      >
        {title && (
          <div
            style={{
              fontSize: 16,
              fontWeight: 700,
              color: 'var(--text)',
              fontFamily: 'var(--font-ui)',
            }}
          >
            {title}
          </div>
        )}
        <div
          style={{
            fontSize: 14,
            color: 'var(--text-2)',
            lineHeight: 1.45,
          }}
        >
          {message}
        </div>
        <div
          style={{
            display: 'flex',
            gap: 10,
            justifyContent: 'flex-end',
            marginTop: 4,
          }}
        >
          <Button size="sm" variant="outline" onClick={onCancel}>
            {cancelLabel}
          </Button>
          <Button
            size="sm"
            variant={danger ? 'danger' : 'primary'}
            onClick={onConfirm}
            autoFocus
          >
            {confirmLabel}
          </Button>
        </div>
      </dialog>
    </div>
  )
}
