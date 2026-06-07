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
}: ConfirmModalProps) {
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
      onClick={onCancel}
      role="dialog"
      aria-modal="true"
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(8,10,14,0.45)',
        backdropFilter: 'blur(2px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 10000,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 'min(420px, calc(100vw - 48px))',
          background: 'var(--surface)',
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
      </div>
    </div>
  )
}
