import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '../components/Icon'
import { IconBtn } from '../components/ui/IconBtn'
import { Tag } from '../components/ui/Tag'
import { Avatar } from '../components/ui/Avatar'
import { Button } from '../components/ui/Button'
import type { Meeting } from '../types'

type TFunction = (key: string) => string

export interface HomeScreenProps {
  t: TFunction
  userFirstName: string
  instanceHost: string | null
  meetings: Meeting[]
  notifBadge: number
  /** When false (anonymous user or no OIDC), the New Meeting hero card is
   * hidden and the Join card expands to fill the row. */
  showNewMeeting: boolean
  /** 'calendar' renders a full-page meetings list with refresh action. */
  mode: 'main' | 'calendar'
  meetInstances: string[]
  authenticatedMeetInstance: string
  onNewMeeting: () => void
  onJoinByCode: (code: string) => void
  onOpenMeeting: (m: Meeting) => void
  onOpenCalendar: () => void
  onOpenNotifications: () => void
  onOpenInstance: () => void
  onRefreshCalendar: () => void
}

function isOngoing(m: Meeting): boolean {
  const now = Date.now() / 1000
  return m.start_time <= now && m.end_time > now
}

function fmtTime(unix: number): string {
  return new Date(unix * 1000).toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

function fmtMeetingDate(meeting: Meeting, t: TFunction): string {
  const d = new Date(meeting.start_time * 1000)
  const now = new Date()
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  const time = fmtTime(meeting.start_time)
  if (sameDay) return time
  const tomorrow = new Date(now)
  tomorrow.setDate(now.getDate() + 1)
  const isTomorrow =
    d.getFullYear() === tomorrow.getFullYear() &&
    d.getMonth() === tomorrow.getMonth() &&
    d.getDate() === tomorrow.getDate()
  if (isTomorrow) return `${t('home.tomorrow')} ${time}`
  const dateLabel = d.toLocaleDateString(undefined, {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
  })
  return `${dateLabel} ${time}`
}

function eyebrowDate(t: TFunction): string {
  void t
  const d = new Date()
  // Capitalised first letter, e.g. "Mardi 6 juin"
  const s = d.toLocaleDateString(undefined, {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
  })
  return s.charAt(0).toUpperCase() + s.slice(1)
}

function pickAvatars(_m: Meeting): string[] {
  // Backend ne nous renvoie pas la liste des invités — on dérive un placeholder
  // depuis le sujet pour styler la liste (pure UI). Si une commande
  // get_meeting_attendees apparaît plus tard, brancher ici.
  return []
}

function accessTone(
  level: string | undefined,
  t: TFunction
): { icon: string; label: string } {
  switch ((level || '').toLowerCase()) {
    case 'public':
      return { icon: 'globe', label: t('home.access.public') }
    case 'restricted':
    case 'restreint':
      return { icon: 'lock', label: t('home.access.restricted') }
    case 'invite_only':
    case 'invitation':
      return { icon: 'shield', label: t('home.access.invite') }
    default:
      return { icon: 'shield', label: t('home.access.invite') }
  }
}

interface MeetingRowProps {
  t: TFunction
  meeting: Meeting
  last: boolean
  onJoin: () => void
}

function MeetingRow({ t, meeting, last, onJoin }: MeetingRowProps) {
  const live = isOngoing(meeting)
  const access = accessTone(undefined, t)
  const avatars = pickAvatars(meeting)
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 18,
        padding: '14px 0',
        borderBottom: last ? 'none' : '1px solid var(--hair)',
      }}
    >
      <div style={{ width: 120, flexShrink: 0 }}>
        {live ? (
          <Tag tone="live" dot>
            {t('home.upcoming.live')}
          </Tag>
        ) : (
          <span
            className="v-mono"
            style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--text)' }}
          >
            {fmtMeetingDate(meeting, t)}
          </span>
        )}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontSize: 15,
            fontWeight: 600,
            letterSpacing: '-0.01em',
            color: 'var(--text)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {meeting.summary || meeting.server_name}
        </div>
        <div style={{ fontSize: 12.5, color: 'var(--text-3)', marginTop: 2 }}>
          {meeting.server_name}
        </div>
      </div>
      {avatars.length > 0 && (
        <div style={{ display: 'flex' }}>
          {avatars.slice(0, 4).map((n, i) => (
            <div
              key={i}
              style={{
                marginLeft: i ? -9 : 0,
                boxShadow: '0 0 0 2px var(--ring-color)',
                borderRadius: '50%',
              }}
            >
              <Avatar name={n} size={28} />
            </div>
          ))}
        </div>
      )}
      <div style={{ width: 130, display: 'flex', justifyContent: 'flex-end' }}>
        <span className="v-chip" style={{ height: 26 }}>
          <Icon name={access.icon} size={13} /> {access.label}
        </span>
      </div>
      <Button
        size="sm"
        variant={live ? 'primary' : 'outline'}
        onClick={onJoin}
        style={{ flexShrink: 0 }}
      >
        {t('home.join.cta')}
      </Button>
    </div>
  )
}

export function HomeScreen({
  t,
  userFirstName,
  instanceHost,
  meetings,
  notifBadge,
  showNewMeeting,
  mode,
  meetInstances,
  authenticatedMeetInstance,
  onNewMeeting,
  onJoinByCode,
  onOpenMeeting,
  onOpenCalendar,
  onOpenNotifications,
  onOpenInstance,
  onRefreshCalendar,
}: HomeScreenProps) {
  const [joinCode, setJoinCode] = useState('')
  const [search, setSearch] = useState('')
  const [joinStatus, setJoinStatus] = useState<
    'idle' | 'checking' | 'valid' | 'auth_required' | 'not_found'
  >('idle')

  // Debounced slug validation — mirrors the legacy live validator.
  useEffect(() => {
    const trimmed = joinCode.trim().replace(/\/$/, '')
    if (!trimmed) {
      setJoinStatus('idle')
      return
    }
    setJoinStatus('checking')
    const ctl = new AbortController()
    const timer = setTimeout(async () => {
      try {
        const candidates: string[] = []
        if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
          candidates.push(trimmed)
        } else if (trimmed.includes('/')) {
          candidates.push(`https://${trimmed}`)
        } else if (/^[a-z]{3}-[a-z]{4}-[a-z]{3}$/.test(trimmed)) {
          if (authenticatedMeetInstance) {
            candidates.push(`https://${authenticatedMeetInstance}/${trimmed}`)
          }
          for (const inst of meetInstances) {
            const u = `https://${inst}/${trimmed}`
            if (!candidates.includes(u)) candidates.push(u)
          }
        } else {
          try {
            const resolved = await invoke<string | null>(
              'resolve_visio_alias',
              {
                name: trimmed,
              }
            )
            if (resolved) candidates.push(resolved)
          } catch {
            /* ignore */
          }
        }
        if (candidates.length === 0) {
          if (!ctl.signal.aborted) setJoinStatus('not_found')
          return
        }
        for (const url of candidates) {
          if (ctl.signal.aborted) return
          const result = await invoke<{ status: string }>('validate_room', {
            url,
            username: null,
          })
          if (ctl.signal.aborted) return
          if (result.status === 'valid') {
            setJoinStatus('valid')
            return
          }
          if (result.status === 'auth_required') {
            setJoinStatus('auth_required')
            return
          }
        }
        setJoinStatus('not_found')
      } catch {
        if (!ctl.signal.aborted) setJoinStatus('idle')
      }
    }, 450)
    return () => {
      ctl.abort()
      clearTimeout(timer)
    }
  }, [joinCode, authenticatedMeetInstance, meetInstances])

  const isCalendarMode = mode === 'calendar'
  const visibleMeetings = useMemo(() => {
    const limit = isCalendarMode ? meetings.length : 6
    if (!search.trim()) return meetings.slice(0, limit)
    const q = search.toLowerCase()
    return meetings
      .filter(
        (m) =>
          (m.summary || '').toLowerCase().includes(q) ||
          (m.server_name || '').toLowerCase().includes(q)
      )
      .slice(0, limit)
  }, [meetings, search, isCalendarMode])

  // Force a 60s rerender so the "Live" badges stay accurate
  const [, setTick] = useState(0)
  useEffect(() => {
    const id = setInterval(() => setTick((n) => n + 1), 60_000)
    return () => clearInterval(id)
  }, [])

  const submitJoin = () => {
    const code = joinCode.trim()
    if (code) onJoinByCode(code)
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
      {/* top bar */}
      <div
        style={{
          height: 60,
          flexShrink: 0,
          borderBottom: '1px solid var(--border)',
          display: 'flex',
          alignItems: 'center',
          gap: 14,
          padding: '0 28px',
        }}
      >
        <div className="v-input" style={{ maxWidth: 340, height: 40 }}>
          <Icon name="search" size={17} style={{ color: 'var(--text-3)' }} />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t('home.search.placeholder')}
            aria-label={t('home.search.placeholder')}
          />
        </div>
        <div style={{ flex: 1 }} />
        {instanceHost && (
          <button
            onClick={onOpenInstance}
            className="v-chip"
            style={{
              height: 36,
              border: 'none',
              cursor: 'pointer',
              fontFamily: 'var(--font-ui)',
            }}
          >
            <Icon name="globe" size={14} /> {instanceHost}
          </button>
        )}
        <IconBtn
          name="bell"
          dim={40}
          size={19}
          variant="soft"
          badge={notifBadge > 0 ? notifBadge : undefined}
          ariaLabel={t('home.notifications')}
          onClick={onOpenNotifications}
        />
      </div>

      {/* main */}
      <div
        className="v-scroll"
        style={{
          flex: 1,
          padding: '26px 28px',
          display: 'flex',
          flexDirection: 'column',
          gap: 22,
        }}
      >
        <div>
          <div className="v-eyebrow">{eyebrowDate(t)}</div>
          <h1 className="v-h1" style={{ fontSize: 28, marginTop: 6 }}>
            {userFirstName
              ? `${t('home.greeting')}, ${userFirstName}`
              : t('home.greeting.anon')}
          </h1>
        </div>

        {!isCalendarMode && (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: showNewMeeting ? '1.3fr 1fr' : '1fr',
              gap: 18,
            }}
          >
            {showNewMeeting && (
              <button
                onClick={onNewMeeting}
                style={{
                  border: 'none',
                  cursor: 'pointer',
                  textAlign: 'left',
                  background: 'var(--accent)',
                  color: 'var(--on-accent)',
                  borderRadius: 'var(--r-card)',
                  padding: 22,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 18,
                  fontFamily: 'var(--font-ui)',
                  boxShadow:
                    '0 14px 30px color-mix(in oklab, var(--accent) 40%, transparent)',
                }}
              >
                <div
                  style={{
                    width: 56,
                    height: 56,
                    borderRadius: 16,
                    background: 'rgba(255,255,255,0.18)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    flexShrink: 0,
                  }}
                >
                  <Icon name="video" size={26} />
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    style={{
                      fontSize: 19,
                      fontWeight: 700,
                      letterSpacing: '-0.02em',
                    }}
                  >
                    {t('home.newMeeting.title')}
                  </div>
                  <div style={{ fontSize: 14, opacity: 0.85, marginTop: 2 }}>
                    {t('home.newMeeting.sub')}
                  </div>
                </div>
                <Icon name="arrowRight" size={20} style={{ opacity: 0.9 }} />
              </button>
            )}

            <div
              className="v-card flat"
              style={{
                padding: 22,
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'center',
                gap: 12,
              }}
            >
              <div
                style={{ fontSize: 15, fontWeight: 700, color: 'var(--text)' }}
              >
                {t('home.join.title')}
              </div>
              <div className="v-input">
                <Icon
                  name="link"
                  size={17}
                  style={{ color: 'var(--text-3)' }}
                />
                <input
                  value={joinCode}
                  onChange={(e) => setJoinCode(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') submitJoin()
                  }}
                  placeholder={t('home.join.placeholder')}
                  aria-label={t('home.join.placeholder')}
                />
                {joinStatus === 'checking' && (
                  <Icon
                    name="dot"
                    size={10}
                    style={{ color: 'var(--text-3)' }}
                  />
                )}
                {joinStatus === 'valid' && (
                  <Icon
                    name="check"
                    size={16}
                    style={{ color: 'var(--live)' }}
                  />
                )}
                {joinStatus === 'auth_required' && (
                  <Icon
                    name="lock"
                    size={15}
                    style={{ color: 'var(--warn)' }}
                  />
                )}
                {joinStatus === 'not_found' && (
                  <Icon name="x" size={15} style={{ color: 'var(--danger)' }} />
                )}
              </div>
              <Button
                variant={joinStatus === 'valid' ? 'primary' : 'secondary'}
                full
                onClick={submitJoin}
                disabled={
                  joinStatus === 'checking' || joinStatus === 'not_found'
                }
              >
                {t('home.join.cta')}
              </Button>
            </div>
          </div>
        )}

        <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: 12,
            }}
          >
            <div className="v-h2">
              {isCalendarMode ? t('sidebar.calendar') : t('home.upcoming')}
            </div>
            {isCalendarMode ? (
              <button
                onClick={onRefreshCalendar}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  fontSize: 13.5,
                  fontWeight: 600,
                  color: 'var(--accent)',
                  fontFamily: 'var(--font-ui)',
                  padding: 0,
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <Icon name="signal" size={14} /> {t('meetings.refresh')}
              </button>
            ) : (
              <button
                onClick={onOpenCalendar}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  fontSize: 13.5,
                  fontWeight: 600,
                  color: 'var(--accent)',
                  fontFamily: 'var(--font-ui)',
                  padding: 0,
                }}
              >
                {t('home.upcoming.viewCalendar')}
              </button>
            )}
          </div>
          {visibleMeetings.length === 0 ? (
            <div
              className="v-card"
              style={{
                padding: '32px 20px',
                textAlign: 'center',
                color: 'var(--text-3)',
                fontSize: 14,
              }}
            >
              {t('home.upcoming.empty')}
            </div>
          ) : (
            <div className="v-card" style={{ padding: '4px 20px' }}>
              {visibleMeetings.map((m, i) => (
                <MeetingRow
                  key={m.id}
                  t={t}
                  meeting={m}
                  last={i === visibleMeetings.length - 1}
                  onJoin={() => onOpenMeeting(m)}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default HomeScreen
