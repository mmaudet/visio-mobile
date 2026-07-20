import { useCallback, useRef, useState, type ReactNode } from 'react'
import { ConfirmModal } from './ConfirmModal'
import { ConfirmContext, type ConfirmOptions } from './useConfirm'

interface PendingConfirm extends ConfirmOptions {
  resolve: (value: boolean) => void
}

export function ConfirmProvider({
  children,
}: Readonly<{ children: ReactNode }>) {
  const [pending, setPending] = useState<PendingConfirm | null>(null)
  const pendingRef = useRef<PendingConfirm | null>(null)
  pendingRef.current = pending

  const confirm = useCallback((opts: ConfirmOptions) => {
    return new Promise<boolean>((resolve) => {
      setPending({ ...opts, resolve })
    })
  }, [])

  const close = useCallback((value: boolean) => {
    const cur = pendingRef.current
    if (cur) {
      cur.resolve(value)
      setPending(null)
    }
  }, [])

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      {pending && (
        <ConfirmModal
          title={pending.title}
          message={pending.message}
          confirmLabel={pending.confirmLabel}
          cancelLabel={pending.cancelLabel}
          danger={pending.danger}
          onConfirm={() => close(true)}
          onCancel={() => close(false)}
        />
      )}
    </ConfirmContext.Provider>
  )
}
