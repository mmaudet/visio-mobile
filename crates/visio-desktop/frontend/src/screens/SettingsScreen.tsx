import type { ReactNode } from 'react'
import { Icon } from '../components/Icon'
import { Avatar } from '../components/ui/Avatar'
import { Button } from '../components/ui/Button'
import { Tag } from '../components/ui/Tag'
import { Row } from '../components/ui/Row'
import { Toggle } from '../components/ui/Toggle'
import type { ThemeChoice } from '../hooks/useTheme'

type TFunction = (key: string) => string

export interface SettingsScreenProps {
  t: TFunction
  displayName: string
  email: string
  isAuthenticated: boolean
  theme: ThemeChoice
  onChangeTheme: (next: ThemeChoice) => void
  instanceHost: string
  language: string
  onOpenLanguage: () => void
  microphoneLabel: string
  cameraLabel: string
  backgroundLabel: string
  visioLinksEnabled: boolean
  onToggleVisioLinks: (next: boolean) => void
  notificationsEnabled: boolean
  onToggleNotifications: (next: boolean) => void
  onManageAccount: () => void
  onSignOut: () => void
  onOpenMicrophone: () => void
  onOpenCamera: () => void
  onOpenBackground: () => void
  onClearLocalData: () => void
  appVersion: string
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

export function SettingsScreen({
  t,
  displayName,
  email,
  isAuthenticated,
  theme,
  onChangeTheme,
  instanceHost,
  language,
  onOpenLanguage,
  microphoneLabel,
  cameraLabel,
  backgroundLabel,
  visioLinksEnabled,
  onToggleVisioLinks,
  notificationsEnabled,
  onToggleNotifications,
  onManageAccount,
  onSignOut,
  onOpenMicrophone,
  onOpenCamera,
  onOpenBackground,
  onClearLocalData,
  appVersion,
}: SettingsScreenProps) {
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
        {/* Profile */}
        <div
          className="v-card"
          style={{
            padding: 18,
            display: 'flex',
            alignItems: 'center',
            gap: 16,
          }}
        >
          <Avatar name={displayName || 'Visio'} size={52} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div
              style={{
                fontSize: 17,
                fontWeight: 700,
                letterSpacing: '-0.01em',
                color: 'var(--text)',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {displayName || 'Visio'}
            </div>
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
          <Button size="sm" variant="outline" onClick={onManageAccount}>
            {t('settings.account.manage')}
          </Button>
        </div>

        {/* 4 group grid */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: 14,
          }}
        >
          <GCard title={t('settings.section.audioVideo')}>
            <Row
              icon="mic"
              title={t('settings.row.microphone')}
              trailing={<TrailVal value={microphoneLabel} />}
              onClick={onOpenMicrophone}
            />
            <Row
              icon="video"
              title={t('settings.row.camera')}
              trailing={<TrailVal value={cameraLabel} />}
              onClick={onOpenCamera}
            />
            <Row
              icon="sparkle"
              iconBg="var(--accent-soft)"
              onAccent
              title={t('settings.row.background')}
              trailing={<TrailVal value={backgroundLabel} />}
              onClick={onOpenBackground}
            />
            <Row
              icon="video"
              title={t('settings.row.videoQuality')}
              trailing={
                <TrailVal value={t('settings.row.videoQuality.value')} />
              }
              last
            />
          </GCard>

          <GCard title={t('settings.section.roomInstance')}>
            <Row
              icon="globe"
              title={t('settings.row.instance')}
              trailing={<TrailVal value={instanceHost || '—'} mono />}
            />
            <Row
              icon="globe"
              title={t('settings.row.language')}
              trailing={<TrailVal value={language} />}
              onClick={onOpenLanguage}
            />
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
            <Icon name="logout" size={17} /> {t('settings.signOut')}
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
