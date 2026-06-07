import {
  useEffect,
  useRef,
  useState,
  type ReactNode,
  type CSSProperties,
} from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Icon, type IconName } from '../components/Icon'
import { Avatar } from '../components/ui/Avatar'
import { Button } from '../components/ui/Button'
import { Tag } from '../components/ui/Tag'
import { Row } from '../components/ui/Row'
import { Toggle } from '../components/ui/Toggle'
import type { ThemeChoice } from '../types'
import type {
  NativeAudioDevice,
  NativeVideoDevice,
} from '../useDeviceEnumeration'

type TFunction = (key: string) => string

const SUPPORTED_LANGS = ['en', 'fr', 'de', 'es', 'it', 'nl'] as const

const CALENDAR_REFRESH_OPTIONS = [
  { key: 'Minutes5', i18n: 'settings.calendarRefresh.5min' },
  { key: 'Minutes15', i18n: 'settings.calendarRefresh.15min' },
  { key: 'Hour1', i18n: 'settings.calendarRefresh.1h' },
  { key: 'Hours4', i18n: 'settings.calendarRefresh.4h' },
  { key: 'Manual', i18n: 'settings.calendarRefresh.manual' },
] as const

export interface SettingsScreenProps {
  t: TFunction
  // Identity
  displayName: string
  email: string
  isAuthenticated: boolean
  oidcEnabled: boolean
  onChangeDisplayName: (name: string) => void
  // Theme
  theme: ThemeChoice
  onChangeTheme: (next: ThemeChoice) => void
  // Language
  lang: string
  onChangeLanguage: (lang: string) => void
  // Instance
  instanceHost: string
  meetInstances: string[]
  onChangeMeetInstances: (next: string[]) => void
  // Devices
  audioInputs: NativeAudioDevice[]
  videoInputs: NativeVideoDevice[]
  selectedAudioInput: string
  selectedVideoInput: string
  onSelectAudioInput: (name: string) => void
  onSelectVideoInput: (uniqueId: string) => void
  // Background
  backgroundLabel: string
  onOpenBackground: () => void
  // Misc toggles
  visioLinksEnabled: boolean
  onToggleVisioLinks: (next: boolean) => void
  notificationsEnabled: boolean
  onToggleNotifications: (next: boolean) => void
  // Actions
  onManageAccount: () => void
  onSignOut: () => void
  onClearLocalData: () => void
  appVersion: string
  // Translation table (for language picker rendering native labels)
  translations: Record<string, Record<string, string>>
}

interface GCardProps {
  title: string
  children: ReactNode
}
function GCard({ title, children }: GCardProps) {
  return (
    <div>
      <div className="v-eyebrow" style={{ marginBottom: 10 }}>
        {title}
      </div>
      <div className="v-card" style={{ padding: '2px 16px' }}>
        {children}
      </div>
    </div>
  )
}

interface TrailValProps {
  value: string
  mono?: boolean
  showChevron?: boolean
}
function TrailVal({ value, mono, showChevron = true }: TrailValProps) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 6,
        flexShrink: 0,
        maxWidth: 200,
      }}
    >
      <span
        className={mono ? 'v-mono' : ''}
        style={{
          fontSize: 13,
          color: 'var(--text-3)',
          fontWeight: mono ? 600 : 500,
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
        }}
      >
        {value}
      </span>
      {showChevron && (
        <Icon
          name="chevronRight"
          size={16}
          style={{ color: 'var(--text-3)' }}
        />
      )}
    </div>
  )
}

interface PopoverItem {
  key: string
  label: string
  active: boolean
}
interface PopoverMenuProps {
  title: string
  items: PopoverItem[]
  onPick: (key: string) => void
  onClose: () => void
  footer?: ReactNode
  align?: 'left' | 'right'
}
function PopoverMenu({
  title,
  items,
  onPick,
  onClose,
  footer,
  align = 'right',
}: PopoverMenuProps) {
  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      const tgt = e.target as Element | null
      if (!tgt) return
      if (!tgt.closest('[data-settings-popover]')) onClose()
    }
    document.addEventListener('mousedown', onDoc)
    return () => document.removeEventListener('mousedown', onDoc)
  }, [onClose])
  const positional: CSSProperties =
    align === 'right' ? { right: 0 } : { left: 0 }
  return (
    <div
      data-settings-popover
      style={{
        position: 'absolute',
        top: 'calc(100% + 6px)',
        ...positional,
        minWidth: 260,
        background: 'var(--surface)',
        borderRadius: 'var(--r-card)',
        boxShadow: 'var(--shadow-pop)',
        padding: 8,
        zIndex: 30,
        maxHeight: 320,
        overflowY: 'auto',
        border: '1px solid var(--border)',
      }}
    >
      <div
        className="v-eyebrow"
        style={{ padding: '6px 10px 8px', fontSize: 11 }}
      >
        {title}
      </div>
      {items.length === 0 ? (
        <div
          style={{
            padding: '10px 12px',
            fontSize: 13,
            color: 'var(--text-3)',
          }}
        >
          —
        </div>
      ) : (
        items.map((it) => (
          <button
            key={it.key}
            onClick={() => onPick(it.key)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 10,
              width: '100%',
              border: 'none',
              background: 'transparent',
              cursor: 'pointer',
              padding: '9px 10px',
              borderRadius: 8,
              fontSize: 13.5,
              color: 'var(--text)',
              fontFamily: 'var(--font-ui)',
              textAlign: 'left',
            }}
          >
            <Icon
              name="check"
              size={15}
              style={{
                color: it.active ? 'var(--accent)' : 'transparent',
                flexShrink: 0,
              }}
            />
            <span
              style={{
                flex: 1,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {it.label}
            </span>
          </button>
        ))
      )}
      {footer && (
        <div style={{ padding: 6, borderTop: '1px solid var(--hair)' }}>
          {footer}
        </div>
      )}
    </div>
  )
}

interface InlineEditorProps {
  initialValue: string
  placeholder?: string
  onSave: (value: string) => void
  onCancel: () => void
  type?: string
  hint?: string
  t: TFunction
  inputTestId?: string
}
function InlineEditor({
  initialValue,
  placeholder,
  onSave,
  onCancel,
  type = 'text',
  hint,
  t,
  inputTestId,
}: InlineEditorProps) {
  const [value, setValue] = useState(initialValue)
  const inputRef = useRef<HTMLInputElement | null>(null)
  useEffect(() => {
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [])
  return (
    <div
      data-settings-popover
      style={{
        position: 'absolute',
        top: 'calc(100% + 6px)',
        right: 0,
        left: 0,
        background: 'var(--surface)',
        borderRadius: 'var(--r-card)',
        boxShadow: 'var(--shadow-pop)',
        padding: 12,
        zIndex: 30,
        border: '1px solid var(--border)',
      }}
    >
      <div
        className="v-input"
        style={{ marginBottom: hint ? 6 : 8, padding: '0 12px' }}
      >
        <input
          ref={inputRef}
          type={type}
          value={value}
          placeholder={placeholder}
          data-testid={inputTestId}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') onSave(value)
            if (e.key === 'Escape') onCancel()
          }}
        />
      </div>
      {hint && (
        <div style={{ fontSize: 12, color: 'var(--text-3)', marginBottom: 8 }}>
          {hint}
        </div>
      )}
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
        <Button size="sm" variant="outline" onClick={onCancel}>
          {t('settings.cancel')}
        </Button>
        <Button size="sm" variant="primary" onClick={() => onSave(value)}>
          {t('settings.save')}
        </Button>
      </div>
    </div>
  )
}

type OpenPopover =
  | null
  | 'name'
  | 'mic'
  | 'cam'
  | 'language'
  | 'instance'
  | 'calendarUrl'
  | 'calendarRefresh'
  | 'profileMenu'

export function SettingsScreen({
  t,
  displayName,
  email,
  isAuthenticated,
  oidcEnabled,
  onChangeDisplayName,
  theme,
  onChangeTheme,
  lang,
  onChangeLanguage,
  instanceHost,
  meetInstances,
  onChangeMeetInstances,
  audioInputs,
  videoInputs,
  selectedAudioInput,
  selectedVideoInput,
  onSelectAudioInput,
  onSelectVideoInput,
  backgroundLabel,
  onOpenBackground,
  visioLinksEnabled,
  onToggleVisioLinks,
  notificationsEnabled,
  onToggleNotifications,
  onManageAccount,
  onSignOut,
  onClearLocalData,
  appVersion,
  translations,
}: SettingsScreenProps) {
  const [open, setOpen] = useState<OpenPopover>(null)
  const [micOnJoin, setMicOnJoin] = useState(true)
  const [camOnJoin, setCamOnJoin] = useState(false)
  const [audioMode, setAudioMode] = useState<'computer' | 'none'>('computer')
  const [adaptiveMode, setAdaptiveMode] = useState(false)
  const [calendarUrl, setCalendarUrl] = useState('')
  const [calendarRefresh, setCalendarRefresh] = useState('Minutes15')
  const [newInstance, setNewInstance] = useState('')

  // Load extra settings on mount.
  useEffect(() => {
    invoke<{
      mic_enabled_on_join?: boolean
      camera_enabled_on_join?: boolean
      audio_mode?: string
      adaptive_mode_enabled?: boolean
    }>('get_settings')
      .then((s) => {
        setMicOnJoin(s.mic_enabled_on_join ?? true)
        setCamOnJoin(s.camera_enabled_on_join ?? false)
        setAudioMode(s.audio_mode === 'none' ? 'none' : 'computer')
        setAdaptiveMode(s.adaptive_mode_enabled ?? false)
      })
      .catch(() => {})
    invoke<string | null>('get_calendar_url')
      .then((url) => setCalendarUrl(url ?? ''))
      .catch(() => {})
    invoke<string>('get_calendar_refresh_interval')
      .then((interval) => setCalendarRefresh(interval))
      .catch(() => {})
  }, [])

  const closeMenu = () => setOpen(null)
  const toggleMenu = (which: OpenPopover) =>
    setOpen((cur) => (cur === which ? null : which))

  // Resolved labels for trailing values.
  const micLabel =
    audioInputs.find((d) => d.name === selectedAudioInput)?.name ||
    t('settings.row.background.off')
  const camLabel =
    videoInputs.find((d) => d.unique_id === selectedVideoInput)?.name ||
    t('settings.row.background.off')
  const languageLabel = t(`lang.${lang}`)
  const calendarRefreshLabel =
    t(
      CALENDAR_REFRESH_OPTIONS.find((o) => o.key === calendarRefresh)?.i18n ||
        'settings.calendarRefresh.15min'
    ) || calendarRefresh
  const audioModeLabel =
    audioMode === 'computer'
      ? t('settings.row.audioMode.computer')
      : t('settings.row.audioMode.none')

  // ---- Handlers -----------------------------------------------------------
  const handleSaveDisplayName = (next: string) => {
    const trimmed = next.trim()
    invoke('set_display_name', { name: trimmed || null }).catch(() => {})
    onChangeDisplayName(trimmed)
    closeMenu()
  }

  const handlePickLanguage = (next: string) => {
    invoke('set_language', { lang: next || null }).catch(() => {})
    onChangeLanguage(next)
    closeMenu()
  }

  const handleToggleMicOnJoin = (next: boolean) => {
    setMicOnJoin(next)
    invoke('set_mic_enabled_on_join', { enabled: next }).catch(() => {})
  }
  const handleToggleCamOnJoin = (next: boolean) => {
    setCamOnJoin(next)
    invoke('set_camera_enabled_on_join', { enabled: next }).catch(() => {})
  }
  const handleSetAudioMode = (next: 'computer' | 'none') => {
    setAudioMode(next)
    invoke('set_audio_mode', { mode: next }).catch(() => {})
  }
  const handleToggleAdaptive = (next: boolean) => {
    setAdaptiveMode(next)
    invoke('set_adaptive_mode_enabled', { enabled: next }).catch(() => {})
  }

  const handleSaveCalendarUrl = (next: string) => {
    const trimmed = next.trim()
    setCalendarUrl(trimmed)
    invoke('set_calendar_url', { url: trimmed || null })
      .then(() => {
        if (trimmed) {
          invoke('refresh_calendar_now').catch(() => {})
        }
      })
      .catch(() => {})
    closeMenu()
  }

  const handlePickCalendarRefresh = (key: string) => {
    setCalendarRefresh(key)
    invoke('set_calendar_refresh_interval', { interval: key }).catch(() => {})
    closeMenu()
  }

  const handleAddInstance = () => {
    const val = newInstance.trim().toLowerCase()
    if (!val || meetInstances.includes(val)) return
    const next = [...meetInstances, val]
    onChangeMeetInstances(next)
    invoke('set_meet_instances', { instances: next }).catch(() => {})
    setNewInstance('')
  }
  const handleRemoveInstance = (inst: string) => {
    const next = meetInstances.filter((x) => x !== inst)
    onChangeMeetInstances(next)
    invoke('set_meet_instances', { instances: next }).catch(() => {})
  }

  return (
    <div
      style={{
        flex: 1,
        minWidth: 0,
        display: 'flex',
        flexDirection: 'column',
        background: 'var(--bg)',
      }}
    >
      <div
        style={{
          height: 60,
          flexShrink: 0,
          borderBottom: '1px solid var(--border)',
          display: 'flex',
          alignItems: 'center',
          padding: '0 28px',
        }}
      >
        <div className="v-h2" style={{ fontSize: 18 }}>
          {t('sidebar.settings')}
        </div>
      </div>

      <div
        className="v-scroll"
        style={{
          flex: 1,
          padding: '16px 28px 20px',
          display: 'flex',
          flexDirection: 'column',
          gap: 14,
        }}
      >
        {/* Profile card */}
        <div
          className="v-card"
          style={{
            padding: 18,
            display: 'flex',
            alignItems: 'center',
            gap: 16,
            position: 'relative',
          }}
        >
          <Avatar name={displayName || 'Visio'} size={52} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <button
              onClick={() => toggleMenu('name')}
              data-testid="settings-display-name-trigger"
              style={{
                background: 'transparent',
                border: 'none',
                padding: 0,
                cursor: 'pointer',
                textAlign: 'left',
                fontSize: 17,
                fontWeight: 700,
                letterSpacing: '-0.01em',
                color: 'var(--text)',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
                maxWidth: '100%',
                display: 'inline-flex',
                alignItems: 'center',
                gap: 6,
                fontFamily: 'var(--font-ui)',
              }}
            >
              <span>{displayName || t('settings.displayName')}</span>
              <Icon
                name="settings"
                size={13}
                style={{ color: 'var(--text-3)' }}
              />
            </button>
            {email && (
              <div
                style={{
                  fontSize: 13,
                  color: 'var(--text-3)',
                  marginTop: 1,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}
              >
                {email}
              </div>
            )}
            {isAuthenticated && (
              <div style={{ marginTop: 8 }}>
                <Tag tone="accent" dot>
                  {/\bgouv\.fr\b/.test(instanceHost)
                    ? 'ProConnect'
                    : t('settings.account.subtitle')}
                </Tag>
              </div>
            )}
          </div>
          {oidcEnabled && (
            <Button size="sm" variant="outline" onClick={onManageAccount}>
              {t('settings.account.manage')}
            </Button>
          )}
          {open === 'name' && (
            <InlineEditor
              t={t}
              initialValue={displayName}
              placeholder={t('home.displayName.placeholder')}
              inputTestId="settings-display-name-input"
              onSave={handleSaveDisplayName}
              onCancel={closeMenu}
            />
          )}
        </div>

        {/* 4 group grid */}
        <div
          style={{
            display: 'grid',
            // Auto-collapses to a single column when the window is too
            // narrow to host both cards side by side — prevents the right
            // column popovers (Instance / Langue / Calendrier) from
            // overflowing offscreen.
            gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
            gap: 14,
          }}
        >
          {/* Audio & vidéo */}
          <GCard title={t('settings.section.audioVideo')}>
            <div style={{ position: 'relative' }}>
              <Row
                icon="mic"
                title={t('settings.row.microphone')}
                trailing={<TrailVal value={micLabel} />}
                onClick={() => toggleMenu('mic')}
              />
              {open === 'mic' && (
                <PopoverMenu
                  title={t('devicePicker.audio.mic')}
                  items={audioInputs.map((d) => ({
                    key: d.name,
                    label: d.name,
                    active: selectedAudioInput === d.name,
                  }))}
                  onPick={(k) => {
                    onSelectAudioInput(k)
                    closeMenu()
                  }}
                  onClose={closeMenu}
                />
              )}
            </div>
            <div style={{ position: 'relative' }}>
              <Row
                icon="video"
                title={t('settings.row.camera')}
                trailing={<TrailVal value={camLabel} />}
                onClick={() => toggleMenu('cam')}
              />
              {open === 'cam' && (
                <PopoverMenu
                  title={t('devicePicker.video.camera')}
                  items={videoInputs.map((d) => ({
                    key: d.unique_id,
                    label: d.name,
                    active: selectedVideoInput === d.unique_id,
                  }))}
                  onPick={(k) => {
                    onSelectVideoInput(k)
                    closeMenu()
                  }}
                  onClose={closeMenu}
                />
              )}
            </div>
            <Row
              icon="sparkle"
              iconBg="var(--accent-soft)"
              onAccent
              title={t('settings.row.background')}
              trailing={<TrailVal value={backgroundLabel} />}
              onClick={onOpenBackground}
            />
            <Row
              icon="mic"
              title={t('settings.row.audioMode')}
              trailing={<TrailVal value={audioModeLabel} />}
              onClick={() =>
                handleSetAudioMode(
                  audioMode === 'computer' ? 'none' : 'computer'
                )
              }
            />
            <Row
              icon="mic"
              title={t('settings.row.micOnJoin')}
              trailing={
                <Toggle on={micOnJoin} onChange={handleToggleMicOnJoin} />
              }
            />
            <Row
              icon="video"
              title={t('settings.row.camOnJoin')}
              trailing={
                <Toggle on={camOnJoin} onChange={handleToggleCamOnJoin} />
              }
              last
            />
          </GCard>

          {/* Salon & instance */}
          <GCard title={t('settings.section.roomInstance')}>
            <div style={{ position: 'relative' }}>
              <Row
                icon="globe"
                title={t('settings.row.instance')}
                trailing={<TrailVal value={instanceHost || '—'} mono />}
                onClick={() => toggleMenu('instance')}
              />
              {open === 'instance' && (
                <PopoverMenu
                  title={t('settings.meetInstances')}
                  items={meetInstances.map((inst) => ({
                    key: inst,
                    label: inst,
                    active: inst === instanceHost,
                  }))}
                  onPick={() => {
                    /* read-only list — items act as labels; remove via buttons below */
                  }}
                  onClose={closeMenu}
                  footer={
                    <div
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 8,
                      }}
                    >
                      {meetInstances.length > 0 && (
                        <div
                          style={{
                            display: 'flex',
                            flexDirection: 'column',
                            gap: 4,
                          }}
                        >
                          {meetInstances.map((inst) => (
                            <div
                              key={`row-${inst}`}
                              style={{
                                display: 'flex',
                                alignItems: 'center',
                                gap: 8,
                                padding: '4px 6px',
                              }}
                            >
                              <span
                                className="v-mono"
                                style={{
                                  flex: 1,
                                  fontSize: 12.5,
                                  color: 'var(--text-2)',
                                  overflow: 'hidden',
                                  textOverflow: 'ellipsis',
                                  whiteSpace: 'nowrap',
                                }}
                              >
                                {inst}
                              </span>
                              <button
                                aria-label={t('action.remove')}
                                onClick={() => handleRemoveInstance(inst)}
                                style={{
                                  background: 'transparent',
                                  border: 'none',
                                  cursor: 'pointer',
                                  color: 'var(--text-3)',
                                  padding: 4,
                                  display: 'inline-flex',
                                  borderRadius: 6,
                                }}
                              >
                                <Icon name="x" size={14} />
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                      <div
                        className="v-input"
                        style={{ padding: '0 10px', height: 36 }}
                      >
                        <input
                          type="text"
                          placeholder={t('settings.instancePlaceholder')}
                          value={newInstance}
                          onChange={(e) => setNewInstance(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') handleAddInstance()
                          }}
                        />
                        <button
                          aria-label={t('action.add')}
                          onClick={handleAddInstance}
                          disabled={!newInstance.trim()}
                          style={{
                            background: 'transparent',
                            border: 'none',
                            cursor: newInstance.trim()
                              ? 'pointer'
                              : 'not-allowed',
                            color: newInstance.trim()
                              ? 'var(--accent)'
                              : 'var(--text-3)',
                            padding: 0,
                            display: 'inline-flex',
                          }}
                        >
                          <Icon name="plus" size={16} />
                        </button>
                      </div>
                    </div>
                  }
                />
              )}
            </div>
            <div style={{ position: 'relative' }}>
              <Row
                icon="globe"
                title={t('settings.row.language')}
                trailing={<TrailVal value={languageLabel} />}
                onClick={() => toggleMenu('language')}
              />
              {open === 'language' && (
                <PopoverMenu
                  title={t('settings.language')}
                  items={SUPPORTED_LANGS.map((code) => ({
                    key: code,
                    label: translations[code]?.[`lang.${code}`] ?? code,
                    active: lang === code,
                  }))}
                  onPick={handlePickLanguage}
                  onClose={closeMenu}
                />
              )}
            </div>
            <div style={{ position: 'relative' }}>
              <Row
                icon="calendar"
                title={t('settings.calendarUrl')}
                sub={calendarUrl || t('settings.calendarUrl.hint')}
                trailing={
                  <Icon
                    name="chevronRight"
                    size={16}
                    style={{ color: 'var(--text-3)' }}
                  />
                }
                onClick={() => toggleMenu('calendarUrl')}
              />
              {open === 'calendarUrl' && (
                <InlineEditor
                  t={t}
                  initialValue={calendarUrl}
                  placeholder="https://cal.example.com/feed.ics"
                  hint={t('settings.calendarUrl.hint')}
                  type="url"
                  onSave={handleSaveCalendarUrl}
                  onCancel={closeMenu}
                />
              )}
            </div>
            <div style={{ position: 'relative' }}>
              <Row
                icon="clock"
                title={t('settings.calendarRefresh')}
                trailing={<TrailVal value={calendarRefreshLabel} />}
                onClick={() => toggleMenu('calendarRefresh')}
              />
              {open === 'calendarRefresh' && (
                <PopoverMenu
                  title={t('settings.calendarRefresh')}
                  items={CALENDAR_REFRESH_OPTIONS.map((opt) => ({
                    key: opt.key,
                    label: t(opt.i18n),
                    active: calendarRefresh === opt.key,
                  }))}
                  onPick={handlePickCalendarRefresh}
                  onClose={closeMenu}
                />
              )}
            </div>
            <Row
              icon="link"
              title={t('settings.row.visioLinks')}
              sub={t('settings.row.visioLinks.sub')}
              trailing={
                <Toggle on={visioLinksEnabled} onChange={onToggleVisioLinks} />
              }
            />
            <Row
              icon="bell"
              title={t('settings.row.notifications')}
              trailing={
                <Toggle
                  on={notificationsEnabled}
                  onChange={onToggleNotifications}
                />
              }
              last
            />
          </GCard>

          {/* Apparence */}
          <GCard title={t('settings.section.appearance')}>
            <div style={{ padding: '12px 0' }}>
              <div className="v-seg">
                <button
                  className={theme === 'system' ? 'on' : ''}
                  onClick={() => onChangeTheme('system')}
                >
                  <Icon name="settings" size={14} />{' '}
                  {t('settings.theme.system')}
                </button>
                <button
                  className={theme === 'light' ? 'on' : ''}
                  onClick={() => onChangeTheme('light')}
                >
                  <Icon name="sun" size={14} /> {t('settings.theme.light')}
                </button>
                <button
                  className={theme === 'dark' ? 'on' : ''}
                  onClick={() => onChangeTheme('dark')}
                >
                  <Icon name="moon" size={14} /> {t('settings.theme.dark')}
                </button>
              </div>
            </div>
          </GCard>

          {/* Confidentialité & sécurité */}
          <GCard title={t('settings.section.privacy')}>
            <Row
              icon="shield"
              iconBg="color-mix(in oklab, var(--live) 15%, var(--surface))"
              title={t('settings.row.e2e')}
              trailing={
                <Tag tone="live" dot>
                  {t('settings.row.e2e.on')}
                </Tag>
              }
            />
            <Row
              icon="lock"
              title={t('settings.row.permissions')}
              trailing={
                <TrailVal value={t('settings.row.permissions.value')} />
              }
            />
            <Row
              icon="signal"
              title={t('settings.row.adaptive')}
              sub={t('settings.row.adaptive.sub')}
              trailing={
                <Toggle on={adaptiveMode} onChange={handleToggleAdaptive} />
              }
            />
            <Row
              icon="x"
              title={t('settings.row.clearData')}
              trailing={
                <Icon
                  name="chevronRight"
                  size={16}
                  style={{ color: 'var(--text-3)' }}
                />
              }
              onClick={onClearLocalData}
              last
            />
          </GCard>
        </div>

        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            marginTop: 'auto',
            paddingTop: 8,
          }}
        >
          <button
            onClick={onSignOut}
            style={{
              background: 'transparent',
              border: 'none',
              cursor: 'pointer',
              color: 'var(--danger)',
              display: 'inline-flex',
              alignItems: 'center',
              gap: 8,
              padding: 0,
              fontFamily: 'var(--font-ui)',
              fontSize: 14,
              fontWeight: 600,
            }}
          >
            <Icon name={'logout' as IconName} size={17} />{' '}
            {t('settings.signOut')}
          </button>
          <span style={{ fontSize: 12, color: 'var(--text-3)' }}>
            {t('settings.footer').replace('{version}', appVersion)}
          </span>
        </div>
      </div>
    </div>
  )
}

export default SettingsScreen
