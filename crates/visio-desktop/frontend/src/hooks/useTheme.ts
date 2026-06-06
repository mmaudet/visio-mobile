import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

export type ThemeChoice = 'system' | 'light' | 'dark'

function applyTheme(choice: ThemeChoice) {
  const root = document.documentElement
  if (choice === 'system') {
    root.removeAttribute('data-theme')
  } else {
    root.setAttribute('data-theme', choice)
  }
}

export function useTheme(initial: ThemeChoice = 'system') {
  const [theme, setThemeState] = useState<ThemeChoice>(initial)

  useEffect(() => {
    applyTheme(theme)
  }, [theme])

  // Réagit aux changements système quand on est en mode "system"
  useEffect(() => {
    if (theme !== 'system') return
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    const onChange = () => applyTheme('system')
    mq.addEventListener('change', onChange)
    return () => mq.removeEventListener('change', onChange)
  }, [theme])

  const setTheme = useCallback((next: ThemeChoice) => {
    setThemeState(next)
    invoke('set_theme', { theme: next }).catch(() => {})
  }, [])

  return { theme, setTheme }
}

export function readInitialTheme(
  rawSetting: string | undefined | null
): ThemeChoice {
  if (
    rawSetting === 'light' ||
    rawSetting === 'dark' ||
    rawSetting === 'system'
  ) {
    return rawSetting
  }
  return 'system'
}
