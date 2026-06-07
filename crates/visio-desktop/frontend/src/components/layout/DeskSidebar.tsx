import type { CSSProperties, ReactNode } from 'react'
import { Icon, type IconName } from '../Icon'
import { Avatar } from '../ui/Avatar'
import { VisioMark } from '../ui/VisioMark'

export type NavKey = 'home' | 'calendar' | 'settings'

export interface SidebarProfile {
  name: string
  subtitle: string
}

export interface DeskSidebarProps {
  active: NavKey
  onNavigate: (key: NavKey) => void
  themeIsDark: boolean
  profile: SidebarProfile
  /**
   * Slot for the "Nouvelle réunion" primary button (caller wires onClick).
   * Pass null to hide it entirely (e.g. when the user isn't authenticated
   * and the create_room flow requires OIDC).
   */
  newMeetingSlot: ReactNode | null
  labels: {
    home: string
    calendar: string
    settings: string
  }
  onProfileClick?: () => void
}

interface NavItemProps {
  icon: string
  label: string
  active: boolean
  onClick: () => void
}

function NavItem({ icon, label, active, onClick }: NavItemProps) {
  const style: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 12,
    height: 40,
    padding: '0 12px',
    borderRadius: 10,
    cursor: 'pointer',
    background: active ? 'var(--accent-soft)' : 'transparent',
    color: active ? 'var(--accent)' : 'var(--text-2)',
    border: 'none',
    width: '100%',
    fontFamily: 'var(--font-ui)',
    fontSize: 14.5,
    fontWeight: active ? 700 : 600,
    letterSpacing: '-0.01em',
    textAlign: 'left',
    transition: 'background .12s, color .12s',
  }
  return (
    <button onClick={onClick} style={style}>
      <Icon name={icon as IconName} size={19} />
      <span style={{ whiteSpace: 'nowrap' }}>{label}</span>
    </button>
  )
}

export function DeskSidebar({
  active,
  onNavigate,
  themeIsDark,
  profile,
  newMeetingSlot,
  labels,
  onProfileClick,
}: DeskSidebarProps) {
  return (
    <aside
      style={{
        width: 250,
        flexShrink: 0,
        borderRight: '1px solid var(--border)',
        background: 'var(--surface)',
        display: 'flex',
        flexDirection: 'column',
        padding: '16px 14px',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          padding: '0 6px',
          marginBottom: 18,
        }}
      >
        <VisioMark size={26} dark={themeIsDark} />
        <span
          style={{
            fontWeight: 600,
            fontSize: 19,
            letterSpacing: '-0.03em',
            color: 'var(--text)',
          }}
        >
          Visio
        </span>
      </div>

      {newMeetingSlot && (
        <div style={{ marginBottom: 18 }}>{newMeetingSlot}</div>
      )}

      <nav style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
        <NavItem
          icon="grid"
          label={labels.home}
          active={active === 'home'}
          onClick={() => onNavigate('home')}
        />
        <NavItem
          icon="calendar"
          label={labels.calendar}
          active={active === 'calendar'}
          onClick={() => onNavigate('calendar')}
        />
      </nav>

      <div style={{ flex: 1 }} />

      <NavItem
        icon="settings"
        label={labels.settings}
        active={active === 'settings'}
        onClick={() => onNavigate('settings')}
      />

      <button
        data-profile-trigger
        onClick={onProfileClick}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 11,
          padding: '12px 8px 4px',
          marginTop: 8,
          borderTop: '1px solid var(--hair)',
          background: 'transparent',
          border: 'none',
          cursor: onProfileClick ? 'pointer' : 'default',
          textAlign: 'left',
          width: '100%',
        }}
      >
        <Avatar name={profile.name} size={36} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div
            style={{
              fontSize: 13.5,
              fontWeight: 700,
              color: 'var(--text)',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {profile.name}
          </div>
          <div
            style={{
              fontSize: 11.5,
              color: 'var(--text-3)',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {profile.subtitle}
          </div>
        </div>
        <Icon name="chevronUp" size={15} style={{ color: 'var(--text-3)' }} />
      </button>
    </aside>
  )
}

export default DeskSidebar
