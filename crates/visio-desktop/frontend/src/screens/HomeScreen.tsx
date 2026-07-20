import { useCallback, useEffect, useMemo, useReducer, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '../components/Icon'
import { Tag } from '../components/ui/Tag'
import { Avatar } from '../components/ui/Avatar'
import { Button } from '../components/ui/Button'
import type { Meeting } from '../types'

type TFunction = (key: string) => string

export interface HomeScreenProps {
  t: TFunction
  userFirstName: string
  instanceHost: string | null
  /** Only show the instance chip in the top bar when the user is actually
   *  authenticated. Showing meetInstances[0] anonymously was misleading. */
  showInstanceChip: boolean
  /** When true (OIDC build, no active session) the top-bar shows a
   *  "Sign in" button that opens Settings → Instances. */
  showSignInCta: boolean
  meetings: Meeting[]
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
  onOpenInstance: () => void
  onRefreshCalendar: () => void
  onSignIn: () => void
  /** Most recent joined visio URLs, freshest first. Rust persists these
   *  in `visio_history`; we surface up to 5 below the "À venir" card. */
  recentVisios?: Array<{ url: string; display_name?: string | null }>
  onOpenRecentVisio?: (url: string) => void
  /** Room URL resolved from a visio:// deep link — prefilled into the join
   *  field once, then cleared via onDeepLinkConsumed. */
  deepLinkUrl: string | null
  onDeepLinkConsumed: () => void
  /** i18n key of an authentication error to surface (e.g. the OIDC callback
   *  watchdog fired), null when there is nothing to show. */
  authError: string | null
  /** Start the system-browser OIDC flow for a Meet instance hostname. */
  onLaunchOidc: (meetInstance: string) => void
  /** Register a one-shot action App runs once the OIDC flow settles (after
   *  the code exchange, or after a launch/exchange/timeout failure). */
  registerPostAuthAction: (fn: (() => void) | null) => void
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

function eyebrowDate(): string {
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

/** Ordered candidate room URLs for a raw join input: full URL, host/path,
 *  bare slug (authenticated instance first, then every known instance,
 *  de-duplicated), or an alias resolved by Rust. */
async function buildJoinCandidates(
  trimmed: string,
  authenticatedMeetInstance: string,
  meetInstances: string[]
): Promise<string[]> {
  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return [trimmed]
  }
  if (trimmed.includes('/')) return [`https://${trimmed}`]
  if (/^[a-z]{3}-[a-z]{4}-[a-z]{3}$/.test(trimmed)) {
    const candidates: string[] = []
    if (authenticatedMeetInstance) {
      candidates.push(`https://${authenticatedMeetInstance}/${trimmed}`)
    }
    for (const inst of meetInstances) {
      const u = `https://${inst}/${trimmed}`
      if (!candidates.includes(u)) candidates.push(u)
    }
    return candidates
  }
  try {
    const resolved = await invoke<string | null>('resolve_visio_alias', {
      name: trimmed,
    })
    return resolved ? [resolved] : []
  } catch {
    return []
  }
}

type JoinProbe =
  | { kind: 'valid' }
  | { kind: 'auth_required'; url: string }
  | { kind: 'not_found' }
  | { kind: 'aborted' }

/** Probe each candidate URL with validate_room until one is joinable. */
async function probeJoinCandidates(
  candidates: string[],
  signal: AbortSignal
): Promise<JoinProbe> {
  for (const url of candidates) {
    if (signal.aborted) return { kind: 'aborted' }
    const result = await invoke<{ status: string }>('validate_room', {
      url,
      username: null,
    })
    if (signal.aborted) return { kind: 'aborted' }
    if (result.status === 'valid') return { kind: 'valid' }
    if (result.status === 'auth_required') return { kind: 'auth_required', url }
  }
  return { kind: 'not_found' }
}

interface MeetingRowProps {
  t: TFunction
  meeting: Meeting
  last: boolean
  onJoin: () => void
}

function MeetingRow({ t, meeting, last, onJoin }: Readonly<MeetingRowProps>) {
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
              key={n}
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
  showInstanceChip,
  showSignInCta,
  meetings,
  showNewMeeting,
  mode,
  meetInstances,
  authenticatedMeetInstance,
  onNewMeeting,
  onJoinByCode,
  onOpenMeeting,
  onOpenCalendar,
  onOpenInstance,
  onRefreshCalendar,
  onSignIn,
  recentVisios = [],
  onOpenRecentVisio,
  deepLinkUrl,
  onDeepLinkConsumed,
  authError,
  onLaunchOidc,
  registerPostAuthAction,
}: Readonly<HomeScreenProps>) {
  const [joinCode, setJoinCode] = useState('')
  const [search, setSearch] = useState('')
  const [joinStatus, setJoinStatus] = useState<
    | 'idle'
    | 'checking'
    | 'valid'
    | 'auth_required'
    | 'authenticating'
    | 'not_found'
  >('idle')
  // Resolved URL of the room that answered auth_required — the sign-in
  // button launches OIDC on its hostname and revalidateRoom re-checks it.
  const [authRoomUrl, setAuthRoomUrl] = useState<string | null>(null)

  // Prefill the join field when a visio:// room deep link arrives.
  useEffect(() => {
    if (deepLinkUrl) {
      setJoinCode(deepLinkUrl)
      onDeepLinkConsumed()
    }
  }, [deepLinkUrl, onDeepLinkConsumed])

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
        const candidates = await buildJoinCandidates(
          trimmed,
          authenticatedMeetInstance,
          meetInstances
        )
        if (candidates.length === 0) {
          if (!ctl.signal.aborted) setJoinStatus('not_found')
          return
        }
        const probe = await probeJoinCandidates(candidates, ctl.signal)
        if (probe.kind === 'aborted') return
        if (probe.kind === 'auth_required') setAuthRoomUrl(probe.url)
        setJoinStatus(probe.kind)
      } catch {
        if (!ctl.signal.aborted) setJoinStatus('idle')
      }
    }, 450)
    return () => {
      ctl.abort()
      clearTimeout(timer)
    }
  }, [joinCode, authenticatedMeetInstance, meetInstances])

  // Sign-in button: start the OIDC flow on the hostname of the room that
  // required authentication. The flow is asynchronous — the exchange code
  // comes back via the visio://auth-callback deep link, then revalidateRoom
  // below runs.
  const handleAuth = () => {
    if (!authRoomUrl) return
    try {
      const url = new URL(
        authRoomUrl.startsWith('http') ? authRoomUrl : `https://${authRoomUrl}`
      )
      setJoinStatus('authenticating')
      onLaunchOidc(url.hostname)
    } catch {
      setJoinStatus('auth_required')
    }
  }

  // Re-validate the room once the OIDC flow settles (successful exchange, or
  // launch/exchange/timeout failure → back to auth_required). Registered
  // with App, which invokes it from the auth-callback handler.
  const revalidateRoom = useCallback(() => {
    if (joinStatus !== 'authenticating' || !authRoomUrl) return
    setJoinStatus('checking')
    invoke<{ status: string }>('validate_room', {
      url: authRoomUrl,
      username: null,
    })
      .then((result) => {
        if (result.status === 'valid') setJoinStatus('valid')
        else if (result.status === 'auth_required')
          setJoinStatus('auth_required')
        else setJoinStatus('not_found')
      })
      .catch(() => setJoinStatus('auth_required'))
  }, [joinStatus, authRoomUrl])

  useEffect(() => {
    registerPostAuthAction(revalidateRoom)
    return () => registerPostAuthAction(null)
  }, [registerPostAuthAction, revalidateRoom])

  const isCalendarMode = mode === 'calendar'
  const visibleMeetings = useMemo(() => {
    if (!search.trim()) {
      return meetings.slice(0, isCalendarMode ? meetings.length : 6)
    }
    const q = search.toLowerCase()
    return meetings.filter(
      (m) =>
        (m.summary || '').toLowerCase().includes(q) ||
        (m.server_name || '').toLowerCase().includes(q)
    )
  }, [meetings, search, isCalendarMode])

  // Force a 60s rerender so the "Live" badges stay accurate
  const [, forceTick] = useReducer((n: number) => n + 1, 0)
  useEffect(() => {
    const id = setInterval(() => forceTick(), 60_000)
    return () => clearInterval(id)
  }, [])

  const submitJoin = () => {
    // The join button is swapped for sign-in in this state — Enter in the
    // input must do the same instead of joining into a silent auth failure.
    if (joinStatus === 'auth_required') {
      handleAuth()
      return
    }
    // An OIDC flow is already running for this room: Enter must not
    // re-submit and kick a duplicate join/launch behind it.
    if (joinStatus === 'authenticating') return
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
        {/* Instance chip only shown when actually connected to that instance.
            Showing meetInstances[0] anonymously was misleading. The bell
            icon is removed until a real notification system exists — it
            only ever displayed a toast with the imminent-meeting count. */}
        {showInstanceChip && instanceHost && (
          <button
            type="button"
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
        {showSignInCta && (
          <Button variant="primary" size="sm" onClick={onSignIn}>
            {t('home.signIn')}
          </Button>
        )}
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
          <div className="v-eyebrow">{eyebrowDate()}</div>
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
                type="button"
                data-testid="home-create-room-button"
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
                  data-testid="home-room-url-input"
                  value={joinCode}
                  onChange={(e) => setJoinCode(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') submitJoin()
                  }}
                  placeholder={t('home.join.placeholder')}
                  aria-label={t('home.join.placeholder')}
                />
                {joinStatus === 'checking' && (
                  <span data-testid="home-room-status">
                    <Icon
                      name="dot"
                      size={10}
                      style={{ color: 'var(--text-3)' }}
                    />
                  </span>
                )}
                {joinStatus === 'valid' && (
                  <span data-testid="home-room-status">
                    <Icon
                      name="check"
                      size={16}
                      style={{ color: 'var(--live)' }}
                    />
                  </span>
                )}
                {joinStatus === 'auth_required' && (
                  <span data-testid="home-room-status">
                    <Icon
                      name="lock"
                      size={15}
                      style={{ color: 'var(--warn)' }}
                    />
                  </span>
                )}
                {joinStatus === 'authenticating' && (
                  <span data-testid="home-room-status">
                    <Icon
                      name="dot"
                      size={10}
                      style={{ color: 'var(--text-3)' }}
                    />
                  </span>
                )}
                {joinStatus === 'not_found' && (
                  <span data-testid="home-room-status">
                    <Icon
                      name="x"
                      size={15}
                      style={{ color: 'var(--danger)' }}
                    />
                  </span>
                )}
              </div>
              {joinStatus === 'authenticating' && (
                <div style={{ fontSize: 13, color: 'var(--text-3)' }}>
                  {t('home.room.authenticating')}
                </div>
              )}
              {joinStatus === 'auth_required' ? (
                <Button
                  data-testid="home-signin-button"
                  variant="primary"
                  full
                  onClick={handleAuth}
                >
                  {t('home.join.authRequired')}
                </Button>
              ) : (
                <Button
                  data-testid="home-join-button"
                  variant={joinStatus === 'valid' ? 'primary' : 'secondary'}
                  full
                  onClick={submitJoin}
                  disabled={
                    joinStatus === 'checking' ||
                    joinStatus === 'not_found' ||
                    joinStatus === 'authenticating'
                  }
                >
                  {t('home.join.cta')}
                </Button>
              )}
              {authError && (
                <div
                  data-testid="home-auth-error"
                  style={{ fontSize: 13, color: 'var(--danger)' }}
                >
                  {t(authError)}
                </div>
              )}
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
                type="button"
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
                type="button"
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

        {/* Recent visios — only on the main mode (calendar mode shows the
            full list of upcoming meetings on its own). */}
        {!isCalendarMode && recentVisios.length > 0 && (
          <div
            style={{ display: 'flex', flexDirection: 'column', minHeight: 0 }}
          >
            <div className="v-h2" style={{ marginBottom: 12 }}>
              {t('home.recentVisios')}
            </div>
            <div className="v-card" style={{ padding: '4px 20px' }}>
              {recentVisios.slice(0, 5).map((entry, i, arr) => {
                const label = entry.display_name || prettyUrl(entry.url)
                return (
                  <div
                    key={entry.url}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 12,
                      padding: '12px 0',
                      borderBottom:
                        i < arr.length - 1 ? '1px solid var(--hair)' : 'none',
                    }}
                  >
                    <Icon
                      name="video"
                      size={18}
                      style={{ color: 'var(--text-3)', flexShrink: 0 }}
                    />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div
                        style={{
                          fontSize: 14,
                          fontWeight: 600,
                          color: 'var(--text)',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {label}
                      </div>
                      <div
                        className="v-mono"
                        style={{
                          fontSize: 12,
                          color: 'var(--text-3)',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {prettyUrl(entry.url)}
                      </div>
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => onOpenRecentVisio?.(entry.url)}
                    >
                      {t('home.join.cta')}
                    </Button>
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function prettyUrl(url: string): string {
  try {
    const u = new URL(url)
    return `${u.host}${u.pathname}`.replace(/\/$/, '')
  } catch {
    return url
  }
}

export default HomeScreen
