import {
  useState,
  useEffect,
  useRef,
  useCallback,
  createContext,
  useContext,
} from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { resolveResource } from '@tauri-apps/api/path'
import { onOpenUrl } from '@tauri-apps/plugin-deep-link'
import {
  RiMicLine,
  RiMicOffLine,
  RiMicOffFill,
  RiVideoOnLine,
  RiVideoOffLine,
  RiArrowUpSLine,
  RiHand,
  RiChat1Line,
  RiGroupLine,
  RiInformationLine,
  RiRecordCircleLine,
  RiFileCopyLine,
  RiCheckLine,
  RiArrowLeftSLine,
  RiFileTextLine,
  RiMailLine,
  RiGlobalLine,
  RiSmartphoneLine,
  RiApps2Line,
  RiArrowRightSLine,
  RiRefreshLine,
  RiPhoneFill,
  RiCloseLine,
  RiSendPlane2Fill,
  RiSettings3Line,
  RiLogoutBoxRLine,
  RiAccountCircleLine,
  RiMore2Fill,
  RiEmotionLine,
  RiFullscreenLine,
  RiFullscreenExitLine,
  RiPushpinLine,
  RiUnpinFill,
  RiVolumeMuteLine,
  RiAddLine,
} from '@remixicon/react'
import {
  useDeviceEnumeration,
  type NativeAudioDevice,
  type NativeVideoDevice,
} from './useDeviceEnumeration'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type View = 'home' | 'lobby' | 'call' | 'settings'

interface Participant {
  sid: string
  identity: string
  name: string | null
  is_muted: boolean
  has_video: boolean
  video_track_sid: string | null
  has_screen_share: boolean
  screen_share_track_sid: string | null
  connection_quality: string
  is_admin?: boolean
}

type FocusItem = {
  participantSid: string
  source: 'camera' | 'screen_share'
} | null

interface DisplayItem {
  key: string
  participant: Participant
  source: 'camera' | 'screen_share'
  trackSid: string | null
  label: string
  isScreenShare: boolean
}

function buildDisplayItems(
  participants: Participant[],
  t: TFunction
): DisplayItem[] {
  const items: DisplayItem[] = []
  for (const p of participants) {
    items.push({
      key: `${p.sid}-camera`,
      participant: p,
      source: 'camera',
      trackSid: p.video_track_sid,
      label: p.name || p.identity || t('unknown'),
      isScreenShare: false,
    })
    if (p.has_screen_share && p.screen_share_track_sid) {
      items.push({
        key: `${p.sid}-screen`,
        participant: p,
        source: 'screen_share',
        trackSid: p.screen_share_track_sid,
        label: p.name || p.identity || t('unknown'),
        isScreenShare: true,
      })
    }
  }
  return items
}

interface ScreenSource {
  id: string
  name: string
  source_type: string
  width: number
  height: number
  thumbnail: string
}

interface ChatMessage {
  id: string
  sender_sid: string
  sender_name: string | null
  text: string
  timestamp_ms: number
}

interface VideoFrame {
  track_sid: string
  data: string // base64 JPEG
  width: number
  height: number
}

interface Settings {
  display_name: string | null
  language: string | null
  mic_enabled_on_join: boolean
  camera_enabled_on_join: boolean
  theme: string
  adaptive_mode_enabled: boolean
  audio_mode: string
  calendar_url?: string | null
  calendar_refresh_interval?: string
}

interface Meeting {
  id: string
  summary: string
  start_time: number
  end_time: number
  room_url: string
  deep_link: string
  server_name: string
}

interface VisioHistoryEntry {
  url: string
  display_name: string | null
}

interface ReactionData {
  id: number
  participantSid: string
  participantName: string
  emoji: string
  timestamp: number
}

const REACTION_EMOJIS: [string, string][] = [
  ['thumbsUp', '\u{1F44D}'],
  ['clap', '\u{1F44F}'],
  ['joy', '\u{1F602}'],
  ['openMouth', '\u{1F62E}'],
  ['tada', '\u{1F389}'],
  ['heart', '\u2764\uFE0F'],
]

// ---------------------------------------------------------------------------
// i18n
// ---------------------------------------------------------------------------

type TFunction = (key: string) => string
const I18nContext = createContext<TFunction>((key) => key)
function useT() {
  return useContext(I18nContext)
}

import en from '../../../../i18n/en.json'
import fr from '../../../../i18n/fr.json'
import de from '../../../../i18n/de.json'
import es from '../../../../i18n/es.json'
import it from '../../../../i18n/it.json'
import nl from '../../../../i18n/nl.json'

const translations: Record<string, Record<string, string>> = {
  en,
  fr,
  de,
  es,
  it,
  nl,
}
const SUPPORTED_LANGS = Object.keys(translations)

// NativeAudioDevice and NativeVideoDevice are imported from
// useDeviceEnumeration.ts

const SLUG_REGEX = /^[a-z]{3}-[a-z]{4}-[a-z]{3}$/

function extractSlug(input: string): string | null {
  const trimmed = input.trim().replace(/\/$/, '')
  const candidate = trimmed.includes('/')
    ? trimmed.split('/').pop() || ''
    : trimmed
  return SLUG_REGEX.test(candidate) ? candidate : null
}

function detectSystemLang(): string {
  const navLang = navigator.language?.split('-')[0]
  return SUPPORTED_LANGS.includes(navLang) ? navLang : 'en'
}

// ---------------------------------------------------------------------------
// Logo SVG tricolore
// ---------------------------------------------------------------------------

function VisioLogo({ size = 64 }: Readonly<{ size?: number }>) {
  // Camera body: 64×54 (ratio ~1.19), centered at x=52
  // Wifi arcs: 3 concentric arcs (r=10,17,24) centered at (52,62), pointing up
  // Stripe: same width as camera body (64), centered on same axis
  const stripeX = 20
  const thirdW = 64 / 3
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 128 128"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className="home-logo"
    >
      {/* Camera body */}
      <rect x="20" y="26" width="64" height="54" rx="10" fill="#000091" />
      {/* Camera lens notch */}
      <path d="M84 44 L108 32 L108 74 L84 62 Z" fill="#000091" />
      {/* Wifi dot */}
      <circle cx="52" cy="62" r="3" fill="#fff" />
      {/* Wifi arc — small (r=10) */}
      <path
        d="M45 55 A10 10 0 0 1 59 55"
        stroke="#fff"
        strokeWidth="3"
        strokeLinecap="round"
        fill="none"
      />
      {/* Wifi arc — medium (r=17) */}
      <path
        d="M40 50 A17 17 0 0 1 64 50"
        stroke="#fff"
        strokeWidth="3"
        strokeLinecap="round"
        fill="none"
      />
      {/* Wifi arc — large (r=24) */}
      <path
        d="M35 45 A24 24 0 0 1 69 45"
        stroke="#fff"
        strokeWidth="3"
        strokeLinecap="round"
        fill="none"
      />
      {/* Tricolore stripe — centered under camera body */}
      <rect
        x={stripeX}
        y="92"
        width={thirdW}
        height="9"
        rx="3"
        fill="#000091"
      />
      <rect
        x={stripeX + thirdW}
        y="92"
        width={thirdW}
        height="9"
        fill="#FFFFFF"
        stroke="#D1D1D6"
        strokeWidth="0.5"
      />
      <rect
        x={stripeX + thirdW * 2}
        y="92"
        width={thirdW}
        height="9"
        rx="3"
        fill="#E1000F"
      />
    </svg>
  )
}

// ---------------------------------------------------------------------------
// Screen Share Icon
// ---------------------------------------------------------------------------

function ScreenShareIcon({ size = 16 }: Readonly<{ size?: number }>) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
      <path d="M21 3H3c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h7v2H8v2h8v-2h-2v-2h7c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 14H3V5h18v12z" />
    </svg>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function getInitials(name: string | null | undefined): string {
  if (!name) return '?'
  const parts = name.trim().split(/\s+/)
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase()
  return name.substring(0, 2).toUpperCase()
}

function getHue(name: string | null | undefined): number {
  return (
    [...(name || '')].reduce((h, c) => h + (c.codePointAt(0) ?? 0), 0) % 360
  )
}

function formatTime(timestampMs: number): string {
  if (!timestampMs) return ''
  const d = new Date(timestampMs)
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

/** Render text with URLs auto-linked. */
function AutoLinkText({ text }: Readonly<{ text: string }>) {
  const parts = text.split(/(https?:\/\/[^\s<]+)/g)
  return (
    <>
      {parts.map((part, i) =>
        /^https?:\/\//.test(part) ? (
          <a
            key={`link-${i}-${part.length}`}
            href={part}
            target="_blank"
            rel="noopener noreferrer"
            className="chat-link"
          >
            {part}
          </a>
        ) : (
          <span key={`text-${i}-${part.length}`}>{part}</span>
        )
      )}
    </>
  )
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

function StatusBadge({ state }: Readonly<{ state: string }>) {
  const t = useT()
  const key = `status.${state}`
  return <span className={`status-badge ${state}`}>{t(key)}</span>
}

// -- Connection Quality Bars ------------------------------------------------

function qualityToBars(quality: string): number {
  if (quality === 'Excellent') return 3
  if (quality === 'Good') return 2
  if (quality === 'Poor') return 1
  return 0
}

function ConnectionQualityBars({ quality }: Readonly<{ quality: string }>) {
  const bars = qualityToBars(quality)
  return (
    <div className="connection-bars">
      {[1, 2, 3].map((n) => (
        <div
          key={n}
          className={`bar ${n <= bars ? 'bar-active' : ''}`}
          style={{ height: `${n * 4 + 2}px` }}
        />
      ))}
    </div>
  )
}

// -- Participant Tile -------------------------------------------------------

interface ParticipantTileProps {
  participant: Participant
  videoFrames: Map<string, string>
  isActiveSpeaker?: boolean
  handRaisePosition?: number
  displayItem?: DisplayItem
  onExpand?: () => void
  bandwidthMode?: string
}

function ParticipantTile({
  participant,
  videoFrames,
  isActiveSpeaker,
  handRaisePosition,
  displayItem,
  onExpand,
  bandwidthMode,
}: Readonly<ParticipantTileProps>) {
  const t = useT()
  const isScreenShare = displayItem?.isScreenShare ?? false
  const trackSid = displayItem
    ? displayItem.trackSid
    : participant.video_track_sid
  let displayName: string
  if (!displayItem) {
    displayName = participant.name || participant.identity || t('unknown')
  } else if (isScreenShare) {
    displayName = `${displayItem.label} (${t('call.screenShare')})`
  } else {
    displayName = displayItem.label
  }
  const initials = getInitials(displayName)
  const hue = getHue(displayName)

  const videoSrc = trackSid ? videoFrames.get(trackSid) : undefined

  // Video paused by bandwidth degradation — show placeholder
  const videoPausedByBw =
    !isScreenShare &&
    participant.has_video &&
    trackSid != null &&
    (bandwidthMode === 'audio_only' ||
      (bandwidthMode === 'reduced_video' && !isActiveSpeaker))

  return (
    <div
      className={`tile ${isActiveSpeaker && !isScreenShare ? 'tile-active-speaker' : ''}`}
      {...(isActiveSpeaker && !isScreenShare
        ? { 'data-testid': `speaker-border:${participant.sid}` }
        : {})}
    >
      {videoSrc && !videoPausedByBw && (
        <img
          className={`tile-video${isScreenShare ? ' tile-video-screen' : ''}`}
          src={`data:image/jpeg;base64,${videoSrc}`}
          alt=""
        />
      )}
      {!videoSrc && isScreenShare && (
        <div className="tile-screen-placeholder">
          <ScreenShareIcon size={48} />
          <span>{t('call.screenShare')}</span>
        </div>
      )}
      {((!videoSrc && !isScreenShare) || videoPausedByBw) && (
        <div className="tile-avatar-no-cam">
          <RiVideoOffLine
            size={32}
            color={videoPausedByBw ? '#ff9800' : undefined}
          />
          <span className="tile-initials-small">{displayName}</span>
          {videoPausedByBw && (
            <span className="tile-bw-paused">{t('bandwidth.videoPaused')}</span>
          )}
        </div>
      )}
      {isScreenShare && onExpand && (
        <button
          className="tile-expand-btn"
          onClick={(e) => {
            e.stopPropagation()
            onExpand()
          }}
          title={t('call.fullscreen')}
        >
          <RiFullscreenLine size={20} />
        </button>
      )}
      <div className="tile-metadata">
        {isScreenShare && (
          <span className="tile-screen-icon">
            <ScreenShareIcon size={14} />
          </span>
        )}
        {!isScreenShare && participant.is_muted && (
          <span className="tile-muted-icon">
            <RiMicOffFill size={14} />
          </span>
        )}
        {!isScreenShare && !participant.is_muted && isActiveSpeaker && (
          <span className="tile-speaking-icon">
            <RiMicLine size={14} />
          </span>
        )}
        {!isScreenShare &&
          handRaisePosition != null &&
          handRaisePosition > 0 && (
            <span className="tile-hand-badge">
              <RiHand size={12} /> {handRaisePosition}
            </span>
          )}
        <span className="tile-name">{displayName}</span>
        <ConnectionQualityBars quality={participant.connection_quality} />
      </div>
    </div>
  )
}

// -- Meetings Tab helpers (module-scope to satisfy S2004) -------------------

function isMeetingImminent(m: Meeting): boolean {
  const nowSec = Date.now() / 1000
  const minutesUntil = (m.start_time - nowSec) / 60
  return minutesUntil >= 0 && minutesUntil < 15
}

function isMeetingOngoing(m: Meeting): boolean {
  const now = Date.now() / 1000
  return m.start_time <= now && now <= m.end_time
}

function formatMeetingRelativeTime(m: Meeting, t: TFunction): string {
  const nowSec = Date.now() / 1000
  const minutesUntil = Math.round((m.start_time - nowSec) / 60)
  const start = new Date(m.start_time * 1000)
  const now = new Date()
  const isToday =
    start.getDate() === now.getDate() &&
    start.getMonth() === now.getMonth() &&
    start.getFullYear() === now.getFullYear()

  if (isMeetingOngoing(m)) {
    const end = new Date(m.end_time * 1000)
    const untilStr = t('meetings.time.until').replace(
      '{time}',
      end.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    )
    return `${t('meetings.time.inProgress')} \u00B7 ${untilStr}`
  }
  if (minutesUntil < 60) {
    return t('meetings.time.inMinutes').replace(
      '{minutes}',
      String(minutesUntil)
    )
  }
  if (minutesUntil < 240) {
    const hours = Math.floor(minutesUntil / 60)
    const mins = minutesUntil % 60
    return mins > 0
      ? t('meetings.time.inHoursMinutes')
          .replace('{hours}', String(hours))
          .replace('{minutes}', mins.toString().padStart(2, '0'))
      : t('meetings.time.inHours').replace('{hours}', String(hours))
  }
  if (isToday) {
    return start.toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    })
  }
  return (
    start.toLocaleDateString([], { weekday: 'short' }) +
    ' ' +
    start.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  )
}

function getDayLabel(ts: number, t: TFunction): string {
  const date = new Date(ts * 1000)
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const meetDay = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  const diffDays = Math.round((meetDay.getTime() - today.getTime()) / 86400000)
  if (diffDays === 0) return t('meetings.today')
  if (diffDays === 1) return t('meetings.tomorrow')
  return date.toLocaleDateString([], {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
  })
}

function groupMeetingsByDay(
  list: Meeting[],
  t: TFunction
): { label: string; meetings: Meeting[] }[] {
  const groups: { label: string; meetings: Meeting[] }[] = []
  for (const m of list) {
    const label = getDayLabel(m.start_time, t)
    const last = groups[groups.length - 1]
    if (last && last.label === label) {
      last.meetings.push(m)
    } else {
      groups.push({ label, meetings: [m] })
    }
  }
  return groups
}

function formatSyncTime(lastSyncTime: Date, t: TFunction): string {
  const diffMs = Date.now() - lastSyncTime.getTime()
  const diffMin = Math.round(diffMs / 60000)
  if (diffMin < 1) return t('meetings.sync').replace('{time}', '< 1 min')
  return t('meetings.sync').replace('{time}', `${diffMin} min`)
}

// -- Meetings Tab -----------------------------------------------------------

function MeetingsTab({
  onJoin,
  displayName,
  onMeetingCountChange,
}: Readonly<{
  onJoin: (meetUrl: string, username: string | null) => void
  displayName: string
  onMeetingCountChange?: (count: number) => void
}>) {
  const t = useT()
  const [status, setStatus] = useState<
    'onboarding' | 'loading' | 'empty' | 'list' | 'error'
  >('onboarding')
  const [meetings, setMeetings] = useState<Meeting[]>([])
  const [calendarUrl, setCalendarUrl] = useState<string | null>(null)
  const [joining, setJoining] = useState<string | null>(null)
  const [loadingMessage, setLoadingMessage] = useState<string>('')
  const [lastSyncTime, setLastSyncTime] = useState<Date | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [syncToast, setSyncToast] = useState<{
    message: string
    isError: boolean
  } | null>(null)

  // Notify parent of meeting count changes
  useEffect(() => {
    onMeetingCountChange?.(meetings.length)
  }, [meetings.length, onMeetingCountChange])

  // Load calendar URL and meetings on mount
  useEffect(() => {
    invoke<string | null>('get_calendar_url')
      .then((url) => {
        setCalendarUrl(url ?? null)
        if (!url) {
          setStatus('onboarding')
          return
        }
        setStatus('loading')
        invoke<Meeting[]>('get_upcoming_meetings')
          .then((list) => {
            setMeetings(list)
            setLastSyncTime(new Date())
            setStatus(list.length === 0 ? 'empty' : 'list')
          })
          .catch(() => setStatus('empty'))
      })
      .catch(() => setStatus('onboarding'))
  }, [])

  // Listen for meetings-updated events
  useEffect(() => {
    let unlistenUpdated: (() => void) | null = null
    let unlistenError: (() => void) | null = null
    listen<Meeting[]>('meetings-updated', (event) => {
      const newMeetings = event.payload
      // Retention guard: if the backend sends an empty list but we had
      // meetings, keep the existing list (defense-in-depth for #126).
      setMeetings((prev) => {
        if (newMeetings.length === 0 && prev.length > 0) {
          return prev
        }
        // Only show sync toast when meetings actually changed
        const prevIds = new Set(prev.map((m) => m.id))
        const newIds = new Set(newMeetings.map((m) => m.id))
        const changed =
          prevIds.size !== newIds.size ||
          [...prevIds].some((id) => !newIds.has(id))
        if (changed) {
          const count = newMeetings.length
          const msg =
            count > 0
              ? t('calendar.sync.success').replace('{count}', String(count))
              : t('calendar.sync.noMeetings')
          setSyncToast({ message: msg, isError: false })
          setTimeout(() => setSyncToast(null), 3000)
        }
        return newMeetings
      })
      setLastSyncTime(new Date())
      if (newMeetings.length > 0) {
        setStatus('list')
      } else {
        setStatus((prev) => (prev === 'list' ? 'list' : 'empty'))
      }
    }).then((fn) => {
      unlistenUpdated = fn
    })
    listen<string>('calendar-error', () => {
      setSyncToast({ message: t('calendar.sync.error'), isError: true })
      setTimeout(() => setSyncToast(null), 4000)
      if (meetings.length === 0) setStatus('error')
    }).then((fn) => {
      unlistenError = fn
    })
    return () => {
      unlistenUpdated?.()
      unlistenError?.()
    }
  }, [])

  const handleRefresh = async () => {
    // Only show full loading screen on initial load (no meetings yet)
    if (meetings.length === 0) {
      setStatus('loading')
      setLoadingMessage(t('meetings.calendar.downloading'))
    }
    setRefreshing(true)
    try {
      await invoke('refresh_calendar_now')
      const list: Meeting[] = await invoke('get_upcoming_meetings')
      setMeetings(list)
      setLastSyncTime(new Date())
      setStatus(list.length === 0 ? 'empty' : 'list')
    } catch {
      setSyncToast({ message: t('calendar.sync.error'), isError: true })
      setTimeout(() => setSyncToast(null), 4000)
      if (meetings.length === 0) {
        setStatus('error')
      }
    } finally {
      setRefreshing(false)
    }
  }

  const handleJoinMeeting = async (m: Meeting) => {
    setJoining(m.id)
    try {
      const uname = displayName.trim() || null
      await invoke('set_display_name', { name: uname })
      onJoin(m.room_url, uname)
    } catch {
      setJoining(null)
    }
  }

  if (status === 'onboarding') {
    return (
      <div className="meetings-onboarding">
        <RiGlobalLine size={48} />
        <p>{t('meetings.onboarding')}</p>
        <p className="meetings-hint">{t('meetings.onboarding.hint')}</p>
      </div>
    )
  }

  if (status === 'loading') {
    return (
      <div className="meetings-loading">
        <span className="meetings-spinner" />
        <p>{loadingMessage || t('meetings.loading')}</p>
      </div>
    )
  }

  if (status === 'empty') {
    return (
      <div className="meetings-empty">
        <p>{t('meetings.empty')}</p>
        <button className="btn btn-secondary" onClick={handleRefresh}>
          {t('meetings.refresh')}
        </button>
      </div>
    )
  }

  if (status === 'error') {
    return (
      <div className="meetings-empty">
        <p>{t('calendar.sync.error')}</p>
        <button className="btn btn-secondary" onClick={handleRefresh}>
          {t('meetings.refresh')}
        </button>
      </div>
    )
  }

  const grouped = groupMeetingsByDay(meetings, t)

  return (
    <div className="meetings-list">
      <div className="meetings-list-header">
        <span className="meetings-count">
          {meetings.length} {t('meetings.count')}
        </span>
        <button
          className={`btn-icon${refreshing ? ' refreshing' : ''}`}
          onClick={handleRefresh}
          disabled={refreshing}
          title={t('meetings.refresh')}
        >
          <RiRefreshLine size={18} />
        </button>
      </div>
      {grouped.map((group) => (
        <div key={group.label} className="meetings-day-group">
          <div className="meetings-day-header">{group.label}</div>
          {group.meetings.map((m) => {
            const ongoing = isMeetingOngoing(m)
            const imminent = isMeetingImminent(m) || ongoing
            return (
              <div
                key={m.id}
                className={`meeting-item${imminent ? ' meeting-imminent' : ''}${ongoing ? ' meeting-ongoing' : ''}`}
              >
                <div className="meeting-info">
                  <span className="meeting-summary">
                    {imminent && (
                      <span
                        className={`meeting-imminent-dot${ongoing ? ' meeting-ongoing-dot' : ''}`}
                      />
                    )}
                    {m.summary || t('meetings.noTitle')}
                  </span>
                  <span className="meeting-time">
                    {formatMeetingRelativeTime(m, t)}
                  </span>
                  <span className="meeting-server">{m.server_name}</span>
                </div>
                <button
                  className="meeting-join-btn"
                  disabled={joining === m.id}
                  onClick={() => handleJoinMeeting(m)}
                >
                  {joining === m.id ? t('home.connecting') : t('home.join')}
                </button>
              </div>
            )
          })}
        </div>
      ))}
      {lastSyncTime && (
        <div className="meetings-sync-footer">
          {formatSyncTime(lastSyncTime, t)}
        </div>
      )}
      {syncToast && (
        <div
          className={`sync-toast ${syncToast.isError ? 'sync-toast-error' : 'sync-toast-success'}`}
        >
          {syncToast.message}
        </div>
      )}
    </div>
  )
}

// -- Home View --------------------------------------------------------------

function HomeView({
  onJoin,
  onOpenSettings,
  displayName,
  onDisplayNameChange,
  deepLinkUrl,
  onDeepLinkConsumed,
  isAuthenticated,
  authenticatedMeetInstance,
  displayNameFromOidc,
  emailFromOidc,
  onLaunchOidc,
  onLogout,
  meetInstances,
}: Readonly<{
  onJoin: (
    meetUrl: string,
    username: string | null,
    roomId?: string,
    accessLevel?: string,
    livekitUrl?: string,
    livekitToken?: string
  ) => void
  onOpenSettings: () => void
  displayName: string
  onDisplayNameChange: (name: string) => void
  deepLinkUrl: string | null
  onDeepLinkConsumed: () => void
  isAuthenticated: boolean
  authenticatedMeetInstance: string
  displayNameFromOidc: string
  emailFromOidc: string
  onLaunchOidc: (meetInstance: string) => void
  onLogout: () => void
  meetInstances: string[]
}>) {
  const t = useT()
  const [activeTab, setActiveTab] = useState<'join' | 'meetings'>('join')
  const [meetUrl, setMeetUrl] = useState('')
  const [roomDisplayName, setRoomDisplayName] = useState('')
  const [resolvedUrl, setResolvedUrl] = useState('')
  const [visioHistory, setRoomHistory] = useState<VisioHistoryEntry[]>([])
  const [meetingCount, setMeetingCount] = useState(0)
  const [tick, setTick] = useState(0)
  const [hasImminentMeeting, setHasImminentMeeting] = useState(false)
  const [reminderToast, setReminderToast] = useState<string | null>(null)

  // Fix 3: Tick every 60 s so countdown labels and imminent state update live
  useEffect(() => {
    const id = setInterval(() => setTick((n) => n + 1), 60_000)
    return () => clearInterval(id)
  }, [])

  useEffect(() => {
    invoke<VisioHistoryEntry[]>('get_visio_history')
      .then(setRoomHistory)
      .catch(() => {})
  }, [])

  // Load meeting count from cache on mount (so badge shows immediately)
  useEffect(() => {
    invoke<Meeting[]>('get_upcoming_meetings')
      .then((list) => {
        setMeetingCount(list.length)
        setHasImminentMeeting(
          list.some((m) => isMeetingImminent(m) || isMeetingOngoing(m))
        )
      })
      .catch(() => {})
  }, [])

  // Also listen for meetings-updated to keep badge in sync even when not on meetings tab
  const meetingsRef = useRef<Meeting[]>([])
  useEffect(() => {
    let unlisten: (() => void) | null = null
    listen<Meeting[]>('meetings-updated', (event) => {
      // Only update badge count if we received meetings; keep previous
      // count when payload is empty (retention guard, #126).
      if (event.payload.length > 0) {
        setMeetingCount(event.payload.length)
        meetingsRef.current = event.payload
        setHasImminentMeeting(
          event.payload.some((m) => isMeetingImminent(m) || isMeetingOngoing(m))
        )
      }
    }).then((fn) => {
      unlisten = fn
    })
    return () => {
      unlisten?.()
    }
  }, [])

  // Fix 3+4: Recompute imminent state on each tick
  useEffect(() => {
    if (meetingsRef.current.length > 0) {
      setHasImminentMeeting(
        meetingsRef.current.some(
          (m) => isMeetingImminent(m) || isMeetingOngoing(m)
        )
      )
    }
  }, [tick])

  // Fix 5: Listen for meeting-reminder events and show notification
  useEffect(() => {
    let unlisten: (() => void) | null = null
    listen<{ summary: string; start_time: number }>(
      'meeting-reminder',
      (event) => {
        const { summary } = event.payload
        const msg = summary
          ? `${summary} — ${t('meetings.time.inMinutes').replace('{minutes}', '15')}`
          : t('meetings.time.inMinutes').replace('{minutes}', '15')
        setReminderToast(msg)
        setTimeout(() => setReminderToast(null), 5000)
        // Also try system notification if available
        if ('Notification' in window && Notification.permission === 'granted') {
          new Notification(summary || t('home.tab.meetings'), {
            body: t('meetings.time.inMinutes').replace('{minutes}', '15'),
          })
        } else if (
          'Notification' in window &&
          Notification.permission === 'default'
        ) {
          Notification.requestPermission()
        }
      }
    ).then((fn) => {
      unlisten = fn
    })
    return () => {
      unlisten?.()
    }
  }, [])

  const [error, setError] = useState('')
  const [joining, setJoining] = useState(false)
  const [roomStatus, setRoomStatus] = useState<
    | 'idle'
    | 'checking'
    | 'valid'
    | 'not_found'
    | 'auth_required'
    | 'authenticating'
    | 'error'
  >('idle')
  const [showServerPicker, setShowServerPicker] = useState(false)
  const [customServer, setCustomServer] = useState('')
  const [showCreateRoom, setShowCreateRoom] = useState(false)

  useEffect(() => {
    if (deepLinkUrl) {
      setMeetUrl(deepLinkUrl)
      onDeepLinkConsumed()
    }
  }, [deepLinkUrl])

  useEffect(() => {
    const trimmed = meetUrl.trim()
    const isSlug = SLUG_REGEX.test(trimmed)

    // Build list of URLs to try
    const urlsToTry: string[] =
      isSlug && meetInstances.length > 0
        ? meetInstances.map((server) => `https://${server}/${trimmed}`)
        : [trimmed]

    const slug = extractSlug(urlsToTry[0])
    if (!slug) {
      // Try alias resolution before giving up
      const candidate = trimmed.includes('/')
        ? trimmed.replace(/\/$/, '').split('/').pop() || trimmed
        : trimmed
      const controller2 = new AbortController()
      const timer2 = setTimeout(async () => {
        try {
          const resolved = await invoke<string | null>('resolve_visio_alias', { name: candidate })
          if (controller2.signal.aborted) return
          if (resolved) {
            setRoomStatus('checking')
            const result = await invoke<{
              status: string
              livekit_url?: string
              token?: string
            }>('validate_room', { url: resolved, username: displayName.trim() || null })
            if (controller2.signal.aborted) return
            if (result.status === 'valid') {
              setRoomStatus('valid')
              setResolvedUrl(resolved)
            } else if (result.status === 'auth_required') {
              setRoomStatus('auth_required')
              setResolvedUrl(resolved)
            } else {
              setRoomStatus('not_found')
              setResolvedUrl(resolved)
            }
          } else {
            setRoomStatus('idle')
            setResolvedUrl(trimmed)
          }
        } catch {
          if (!controller2.signal.aborted) {
            setRoomStatus('idle')
            setResolvedUrl(trimmed)
          }
        }
      }, 500)
      return () => {
        clearTimeout(timer2)
        controller2.abort()
      }
    }
    setRoomStatus('checking')
    const controller = new AbortController()
    const timer = setTimeout(async () => {
      try {
        let foundValid = false
        for (const url of urlsToTry) {
          if (controller.signal.aborted) return
          const result = await invoke<{
            status: string
            livekit_url?: string
            token?: string
          }>('validate_room', { url, username: displayName.trim() || null })
          if (controller.signal.aborted) return
          if (result.status === 'valid') {
            setRoomStatus('valid')
            setResolvedUrl(url)
            foundValid = true
            break
          }
          if (result.status === 'auth_required') {
            setRoomStatus('auth_required')
            setResolvedUrl(url)
            foundValid = true // don't show not_found
            break
          }
        }
        if (!foundValid) {
          setRoomStatus('not_found')
          setResolvedUrl(urlsToTry[0])
        }
      } catch {
        if (!controller.signal.aborted) setRoomStatus('error')
      }
    }, 500)
    return () => {
      clearTimeout(timer)
      controller.abort()
    }
  }, [meetUrl])

  const handleJoin = async () => {
    let url = resolvedUrl
    if (!url) {
      setError(t('home.error.noUrl'))
      return
    }
    const trimmedDisplayName = roomDisplayName.trim()
    if (trimmedDisplayName) {
      const sep = url.includes('?') ? '&' : '?'
      url = `${url}${sep}visio=${encodeURIComponent(trimmedDisplayName)}`
    }
    setError('')
    setJoining(true)
    try {
      const uname = displayName.trim() || null
      await invoke('set_display_name', { name: uname })
      onJoin(url, uname)
    } catch (e) {
      setError(String(e))
      setJoining(false)
    }
  }

  const handleAuth = async () => {
    try {
      // Extract the instance hostname from the resolved URL
      const url = new URL(
        resolvedUrl.startsWith('http') ? resolvedUrl : `https://${resolvedUrl}`
      )
      setRoomStatus('authenticating')
      await invoke('start_oidc_auth', { meetInstance: url.hostname })
      // After auth, re-trigger validation by bumping state
      setRoomStatus('checking')
      const result = await invoke<{ status: string }>('validate_room', {
        url: resolvedUrl,
        username: displayName.trim() || null,
      })
      if (result.status === 'valid') setRoomStatus('valid')
      else if (result.status === 'auth_required') setRoomStatus('auth_required')
      else setRoomStatus('error')
    } catch (e) {
      setError(String(e))
      setRoomStatus('auth_required')
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleJoin()
  }

  return (
    <div id="home" className="section active">
      <div className="home-tabs">
        <div className="home-tabs-group">
          <button
            className={`home-tab${activeTab === 'join' ? ' home-tab-active' : ''}`}
            onClick={() => setActiveTab('join')}
          >
            {t('home.tab.join')}
          </button>
          <button
            className={`home-tab${activeTab === 'meetings' ? ' home-tab-active' : ''}`}
            onClick={() => setActiveTab('meetings')}
          >
            {t('home.tab.meetings')}
            {meetingCount > 0 && (
              <span
                className={`tab-badge${hasImminentMeeting ? ' tab-badge-imminent' : ''}`}
              >
                {meetingCount}
              </span>
            )}
          </button>
        </div>
        <button
          className="settings-gear"
          onClick={onOpenSettings}
          data-testid="home-settings-button"
        >
          <RiSettings3Line size={20} />
        </button>
      </div>
      <div className="home-tab-content">
        {activeTab === 'meetings' ? (
          <MeetingsTab
            onJoin={onJoin}
            displayName={displayName}
            onMeetingCountChange={setMeetingCount}
          />
        ) : (
          <div className="join-form">
            <img src="/logo.png?v=2" alt="Visio Mobile" className="home-logo" />
            <h2>{t('app.title')}</h2>
            {isAuthenticated ? (
              <div className="auth-card">
                <div className="auth-avatar">
                  {(() => {
                    const parts = displayNameFromOidc
                      .split(' ')
                      .filter(Boolean)
                      .slice(0, 2)
                    const initials = parts
                      .map((p) => p[0]?.toUpperCase())
                      .join('')
                    return initials || emailFromOidc?.[0]?.toUpperCase() || '?'
                  })()}
                </div>
                <div className="auth-info">
                  <span className="auth-name">
                    {displayNameFromOidc || emailFromOidc}
                  </span>
                  {emailFromOidc && displayNameFromOidc && (
                    <span className="auth-email">{emailFromOidc}</span>
                  )}
                </div>
                <button
                  className="auth-logout"
                  onClick={onLogout}
                  title={t('home.logout')}
                >
                  <RiLogoutBoxRLine size={20} />
                </button>
              </div>
            ) : (
              <div className="auth-status">
                <button
                  className="btn btn-primary"
                  data-testid="home-connect-button"
                  onClick={() => {
                    if (meetInstances.length <= 1) {
                      if (meetInstances.length > 0)
                        onLaunchOidc(meetInstances[0])
                    } else {
                      setCustomServer('')
                      setShowServerPicker(true)
                    }
                  }}
                >
                  <RiAccountCircleLine size={18} /> {t('home.connect')}
                </button>
                {showServerPicker && (
                  <div
                    className="server-picker-overlay"
                    onClick={() => setShowServerPicker(false)}
                  >
                    <div
                      className="server-picker"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <h3>{t('home.serverPicker.title')}</h3>
                      <div className="server-list">
                        {meetInstances.map((instance) => (
                          <button
                            key={instance}
                            className="server-item"
                            onClick={() => {
                              setShowServerPicker(false)
                              onLaunchOidc(instance)
                            }}
                          >
                            {instance}
                          </button>
                        ))}
                      </div>
                      <div className="server-custom">
                        <input
                          type="text"
                          placeholder="meet.example.com"
                          value={customServer}
                          onChange={(e) => setCustomServer(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' && customServer.trim()) {
                              setShowServerPicker(false)
                              onLaunchOidc(customServer.trim())
                            }
                          }}
                        />
                        <button
                          className="btn btn-secondary"
                          disabled={!customServer.trim()}
                          onClick={() => {
                            if (customServer.trim()) {
                              setShowServerPicker(false)
                              onLaunchOidc(customServer.trim())
                            }
                          }}
                        >
                          {t('home.connect')}
                        </button>
                      </div>
                      <button
                        className="btn btn-cancel"
                        onClick={() => setShowServerPicker(false)}
                      >
                        {t('home.serverPicker.cancel')}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
            <div className="form-group">
              <label htmlFor="meetUrl">{t('home.meetUrl')}</label>
              <input
                id="meetUrl"
                type="text"
                placeholder="abc-defg-hij"
                autoComplete="off"
                data-testid="home-room-url-input"
                value={meetUrl}
                onChange={(e) => setMeetUrl(e.target.value)}
                onKeyDown={handleKeyDown}
              />
              {roomStatus === 'checking' && (
                <div
                  className="room-status checking"
                  data-testid="home-room-status"
                >
                  {t('home.room.checking')}
                </div>
              )}
              {roomStatus === 'valid' && (
                <div
                  className="room-status valid"
                  data-testid="home-room-status"
                >
                  {t('home.room.valid')}
                </div>
              )}
              {roomStatus === 'not_found' && (
                <div
                  className="room-status not-found"
                  data-testid="home-room-status"
                >
                  {t('home.room.notFound')}
                </div>
              )}
              {roomStatus === 'auth_required' && (
                <div
                  className="room-status auth-required"
                  data-testid="home-room-status"
                >
                  {t('home.room.authRequired')}
                </div>
              )}
              {roomStatus === 'authenticating' && (
                <div
                  className="room-status checking"
                  data-testid="home-room-status"
                >
                  {t('home.room.authenticating')}
                </div>
              )}
              {roomStatus === 'error' && (
                <div
                  className="room-status error"
                  data-testid="home-room-status"
                >
                  {t('home.room.error')}
                </div>
              )}
            </div>
            <div className="form-group">
              <label htmlFor="username">{t('home.displayName')}</label>
              <input
                id="username"
                type="text"
                placeholder={t('home.displayName.placeholder')}
                autoComplete="off"
                data-testid="home-display-name-input"
                value={displayName}
                onChange={(e) => onDisplayNameChange(e.target.value)}
                onKeyDown={handleKeyDown}
              />
            </div>
            <div className="form-group">
              <label htmlFor="roomDisplayName">
                {t('home.roomDisplayName')}
              </label>
              <input
                id="roomDisplayName"
                type="text"
                placeholder={t('home.roomDisplayNamePlaceholder')}
                autoComplete="off"
                data-testid="home-room-input"
                value={roomDisplayName}
                onChange={(e) => setRoomDisplayName(e.target.value)}
                onKeyDown={handleKeyDown}
              />
            </div>
            {roomStatus === 'auth_required' ? (
              <button className="btn btn-primary" onClick={handleAuth}>
                {t('home.signIn')}
              </button>
            ) : (
              <button
                className="btn btn-primary"
                disabled={joining || roomStatus !== 'valid'}
                onClick={handleJoin}
                data-testid="home-join-button"
              >
                {joining ? t('home.connecting') : t('home.join')}
              </button>
            )}
            {isAuthenticated && authenticatedMeetInstance && (
              <button
                className="btn btn-primary"
                style={{
                  marginTop: '8px',
                  background: 'var(--bg-tertiary)',
                  color: 'var(--text)',
                }}
                onClick={() => setShowCreateRoom(true)}
                data-testid="home-create-room-button"
              >
                {t('home.createRoom')}
              </button>
            )}
            {roomStatus === 'auth_required' && (
              <div
                className="room-status auth-required"
                data-testid="home-room-status"
              >
                {t('home.room.authRequired')}
              </div>
            )}
            {roomStatus === 'authenticating' && (
              <div
                className="room-status checking"
                data-testid="home-room-status"
              >
                {t('home.room.authenticating')}
              </div>
            )}
            {roomStatus === 'error' && (
              <div className="room-status error" data-testid="home-room-status">
                {t('home.room.error')}
              </div>
            )}
            <div className="error-msg">{error}</div>
            {visioHistory.length > 0 && (
              <div className="room-history">
                <h4>{t('home.recentRooms')}</h4>
                {visioHistory.map((entry) => {
                  const { url, display_name } = entry
                  const slug = url.includes('/') ? url.split('/').pop() : url
                  let host: string
                  try {
                    host = new URL(url).host
                  } catch {
                    host = url
                  }
                  const primaryLabel = display_name || slug || url
                  const secondaryLabel = display_name
                    ? `${slug} · ${host}`
                    : host
                  return (
                    <button
                      key={url}
                      className="room-history-item"
                      disabled={joining}
                      onClick={async () => {
                        setMeetUrl(url)
                        setError('')
                        setJoining(true)
                        try {
                          const uname = displayName.trim() || null
                          const result = await invoke<{ status: string }>(
                            'validate_room',
                            { url, username: uname }
                          )
                          if (result.status === 'valid') {
                            await invoke('set_display_name', { name: uname })
                            onJoin(url, uname)
                          } else {
                            // Validation failed — fall back to filling the URL field so the user can see the status
                            setJoining(false)
                          }
                        } catch (e) {
                          setError(String(e))
                          setJoining(false)
                        }
                      }}
                      data-testid={`home_room_history_item_${url}`}
                    >
                      {joining && meetUrl === url ? (
                        <span className="room-history-spinner" />
                      ) : (
                        <RiGlobalLine size={16} />
                      )}
                      <div className="room-history-info">
                        <span className="room-history-slug">
                          {primaryLabel}
                        </span>
                        {secondaryLabel && (
                          <span className="room-history-host">
                            {secondaryLabel}
                          </span>
                        )}
                      </div>
                    </button>
                  )
                })}
              </div>
            )}
          </div>
        )}
      </div>
      {showCreateRoom && authenticatedMeetInstance && (
        <CreateRoomDialog
          meetInstance={authenticatedMeetInstance}
          onCreated={async (
            createdUrl,
            roomId,
            accessLevel,
            livekitUrl,
            livekitToken
          ) => {
            setShowCreateRoom(false)
            const uname = displayName.trim() || null
            try {
              await invoke('set_display_name', { name: uname })
              onJoin(
                createdUrl,
                uname,
                roomId,
                accessLevel,
                livekitUrl,
                livekitToken
              )
            } catch (e) {
              setError(String(e))
            }
          }}
          onCancel={() => setShowCreateRoom(false)}
        />
      )}
      {reminderToast && (
        <div className="sync-toast sync-toast-success">{reminderToast}</div>
      )}
    </div>
  )
}

// -- Create Room Dialog -----------------------------------------------------

function CreateRoomDialog({
  meetInstance,
  onCreated,
  onCancel,
}: Readonly<{
  meetInstance: string
  onCreated: (
    meetUrl: string,
    roomId?: string,
    accessLevel?: string,
    livekitUrl?: string,
    livekitToken?: string
  ) => void
  onCancel: () => void
}>) {
  const t = useT()
  const [accessLevel, setAccessLevel] = useState<
    'public' | 'trusted' | 'restricted'
  >('public')
  const [roomDisplayName, setRoomDisplayName] = useState('')
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState('')
  const [createdUrl, setCreatedUrl] = useState('')
  const [copiedHttp, setCopiedHttp] = useState(false)
  const [copiedDeep, setCopiedDeep] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<any[]>([])
  const [invitedUsers, setInvitedUsers] = useState<any[]>([])
  const [searching, setSearching] = useState(false)
  const [createdRoomId, setCreatedRoomId] = useState('')
  const [createdLivekitUrl, setCreatedLivekitUrl] = useState('')
  const [createdLivekitToken, setCreatedLivekitToken] = useState('')
  const [aliasConflictName, setAliasConflictName] = useState('')
  const [aliasConflictUrl, setAliasConflictUrl] = useState('')

  const deepLink = createdUrl
    ? `visio://${createdUrl.replace(/^https?:\/\//, '')}`
    : ''

  useEffect(() => {
    if (searchQuery.length < 3) {
      setSearchResults([])
      return
    }
    const timer = setTimeout(async () => {
      setSearching(true)
      try {
        const results = await invoke<any[]>('search_users', {
          query: searchQuery,
        })
        setSearchResults(
          results.filter(
            (u: any) => !invitedUsers.some((inv: any) => inv.id === u.id)
          )
        )
      } catch {
        setSearchResults([])
      }
      setSearching(false)
    }, 300)
    return () => clearTimeout(timer)
  }, [searchQuery, invitedUsers])

  const handleCreate = async () => {
    setCreating(true)
    setError('')
    const meetUrl = `https://${meetInstance}`
    try {
      const result = await invoke<{
        slug: string
        id: string
        livekit_url?: string
        livekit_token?: string
      }>('create_room', {
        meetUrl,
        accessLevel,
      })
      const trimmedName = roomDisplayName.trim()
      const baseUrl = `${meetUrl}/${result.slug}`
      setCreatedUrl(
        trimmedName
          ? `${baseUrl}?visio=${encodeURIComponent(trimmedName)}`
          : baseUrl
      )
      if (trimmedName) {
        const conflict = await invoke<string | null>('check_visio_alias_conflict', {
          name: trimmedName,
          url: baseUrl,
        })
        if (conflict) {
          setAliasConflictName(trimmedName)
          setAliasConflictUrl(baseUrl)
        } else {
          await invoke('add_visio_alias', { name: trimmedName, url: baseUrl }).catch(() => {})
        }
      }
      setCreatedRoomId(result.id)
      setCreatedLivekitUrl(result.livekit_url ?? '')
      setCreatedLivekitToken(result.livekit_token ?? '')
      if (accessLevel === 'restricted') {
        for (const user of invitedUsers) {
          try {
            await invoke('add_access', { userId: user.id, roomId: result.id })
          } catch (e) {
            console.warn('Failed to add access for', user.email, e)
          }
        }
      }
    } catch (e) {
      setError(t('home.createRoom.error') + ': ' + String(e))
    } finally {
      setCreating(false)
    }
  }

  const handleCopy = async (text: string, setFn: (v: boolean) => void) => {
    try {
      await navigator.clipboard.writeText(text)
      setFn(true)
      setTimeout(() => setFn(false), 2000)
    } catch {
      /* ignore */
    }
  }

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div
        className="settings-modal create-room-dialog"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="settings-header">
          <span>{t('home.createRoom')}</span>
          <button onClick={onCancel}>
            <RiCloseLine size={20} />
          </button>
        </div>
        <div className="settings-body">
          {!createdUrl ? (
            <>
              <div className="form-field">
                <label>{t('home.roomDisplayName')}</label>
                <input
                  type="text"
                  className="info-link-input"
                  placeholder={t('home.roomDisplayNamePlaceholder')}
                  value={roomDisplayName}
                  onChange={(e) => setRoomDisplayName(e.target.value)}
                  autoComplete="off"
                />
              </div>
              <div className="form-field">
                <label>{t('home.createRoom.access')}</label>
                <div className="access-level-options">
                  <label
                    className={`access-option ${accessLevel === 'public' ? 'selected' : ''}`}
                    htmlFor="access-public"
                  >
                    <input
                      type="radio"
                      id="access-public"
                      name="accessLevel"
                      value="public"
                      checked={accessLevel === 'public'}
                      onChange={() => setAccessLevel('public')}
                    />
                    <div>
                      <div style={{ fontWeight: 500, fontSize: '0.9rem' }}>
                        {t('home.createRoom.public')}
                      </div>
                      <div
                        style={{
                          fontSize: '0.78rem',
                          color: 'var(--text-secondary)',
                        }}
                      >
                        {t('home.createRoom.publicDesc')}
                      </div>
                    </div>
                  </label>
                  <label
                    className={`access-option ${accessLevel === 'trusted' ? 'selected' : ''}`}
                    htmlFor="access-trusted"
                  >
                    <input
                      type="radio"
                      id="access-trusted"
                      name="accessLevel"
                      value="trusted"
                      checked={accessLevel === 'trusted'}
                      onChange={() => setAccessLevel('trusted')}
                    />
                    <div>
                      <div style={{ fontWeight: 500, fontSize: '0.9rem' }}>
                        {t('home.createRoom.trusted')}
                      </div>
                      <div
                        style={{
                          fontSize: '0.78rem',
                          color: 'var(--text-secondary)',
                        }}
                      >
                        {t('home.createRoom.trustedDesc')}
                      </div>
                    </div>
                  </label>
                  <label
                    className={`access-option ${accessLevel === 'restricted' ? 'selected' : ''}`}
                    htmlFor="access-restricted"
                  >
                    <input
                      type="radio"
                      id="access-restricted"
                      name="accessLevel"
                      value="restricted"
                      checked={accessLevel === 'restricted'}
                      onChange={() => setAccessLevel('restricted')}
                    />
                    <div>
                      <div style={{ fontWeight: 500, fontSize: '0.9rem' }}>
                        {t('home.createRoom.restricted')}
                      </div>
                      <div
                        style={{
                          fontSize: '0.78rem',
                          color: 'var(--text-secondary)',
                        }}
                      >
                        {t('home.createRoom.restrictedDesc')}
                      </div>
                    </div>
                  </label>
                </div>
              </div>
              {accessLevel === 'restricted' && (
                <div className="form-field" style={{ marginTop: '8px' }}>
                  <label>{t('restricted.invite')}</label>
                  <input
                    type="text"
                    className="info-link-input"
                    placeholder={t('restricted.searchUsers')}
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                  />
                  {searchResults.length > 0 && (
                    <div className="search-dropdown">
                      {searchResults.map((user: any) => (
                        <button
                          key={user.id}
                          type="button"
                          className="search-result"
                          onClick={() => {
                            setInvitedUsers([...invitedUsers, user])
                            setSearchQuery('')
                            setSearchResults([])
                          }}
                        >
                          <span className="search-name">
                            {user.full_name || user.email}
                          </span>
                          <span className="search-email">{user.email}</span>
                        </button>
                      ))}
                    </div>
                  )}
                  {invitedUsers.length > 0 && (
                    <div className="invited-chips">
                      {invitedUsers.map((user: any) => (
                        <span key={user.id} className="user-chip">
                          {user.full_name || user.email}
                          <button
                            className="chip-remove"
                            onClick={() =>
                              setInvitedUsers(
                                invitedUsers.filter(
                                  (u: any) => u.id !== user.id
                                )
                              )
                            }
                          >
                            ×
                          </button>
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              )}
              {error && <div className="create-room-error">{error}</div>}
            </>
          ) : (
            <div
              className="form-field"
              style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}
            >
              <label>{t('settings.incall.roomInfo')}</label>
              <div className="info-link-header">
                <RiGlobalLine size={16} />
                <span>{t('settings.incall.roomLink')}</span>
                <button
                  className="info-copy-icon"
                  onClick={() => handleCopy(createdUrl, setCopiedHttp)}
                  title={t('settings.incall.copied')}
                >
                  {copiedHttp ? (
                    <RiCheckLine size={16} />
                  ) : (
                    <RiFileCopyLine size={16} />
                  )}
                </button>
              </div>
              <input
                className="info-link-input"
                readOnly
                value={createdUrl}
                onClick={(e) => (e.target as HTMLInputElement).select()}
              />
              <div className="info-link-header">
                <RiSmartphoneLine size={16} />
                <span>{t('settings.incall.deepLink')}</span>
                <button
                  className="info-copy-icon"
                  onClick={() => handleCopy(deepLink, setCopiedDeep)}
                  title={t('settings.incall.copied')}
                >
                  {copiedDeep ? (
                    <RiCheckLine size={16} />
                  ) : (
                    <RiFileCopyLine size={16} />
                  )}
                </button>
              </div>
              <input
                className="info-link-input"
                readOnly
                value={deepLink}
                onClick={(e) => (e.target as HTMLInputElement).select()}
              />
              {roomDisplayName.trim() && (() => {
                const host = createdUrl.replace(/^https?:\/\//, '').split('/')[0]
                const simplifiedUrl = `visio://${host}/${roomDisplayName.trim()}`
                return (
                  <>
                    <div className="info-link-header" style={{ marginTop: '8px' }}>
                      <RiGlobalLine size={16} />
                      <span>{t('home.createVisio.simplifiedUrl')}</span>
                      <button
                        className="info-copy-icon"
                        onClick={() => handleCopy(simplifiedUrl, setCopiedDeep)}
                        title={t('settings.incall.copied')}
                      >
                        <RiFileCopyLine size={16} />
                      </button>
                    </div>
                    <input
                      className="info-link-input"
                      readOnly
                      value={simplifiedUrl}
                      onClick={(e) => (e.target as HTMLInputElement).select()}
                    />
                    <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
                      {t('home.createVisio.simplifiedUrlHint')}
                    </span>
                  </>
                )
              })()}
            </div>
          )}
        </div>
        <div
          style={{
            display: 'flex',
            gap: '8px',
            padding: '0 20px 20px',
            justifyContent: 'flex-end',
          }}
        >
          <button className="btn btn-cancel" onClick={onCancel}>
            {t('home.serverPicker.cancel')}
          </button>
          {!createdUrl ? (
            <button
              className="btn btn-primary"
              style={{ width: 'auto' }}
              disabled={creating}
              onClick={handleCreate}
            >
              {creating
                ? t('home.createRoom.creating')
                : t('home.createRoom.create')}
            </button>
          ) : (
            <button
              className="btn btn-primary"
              style={{ width: 'auto' }}
              onClick={() =>
                onCreated(
                  createdUrl,
                  createdRoomId,
                  accessLevel,
                  createdLivekitUrl,
                  createdLivekitToken
                )
              }
            >
              {t('home.join')}
            </button>
          )}
        </div>
      </div>
      {aliasConflictName && (
        <div className="modal-overlay" onClick={() => { setAliasConflictName(''); setAliasConflictUrl('') }}>
          <div className="settings-modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 400 }}>
            <div className="settings-header">
              <span>{t('alias.conflictTitle').replace('{name}', aliasConflictName)}</span>
            </div>
            <div className="settings-footer" style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', padding: '16px' }}>
              <button className="btn" onClick={() => { setAliasConflictName(''); setAliasConflictUrl('') }}>
                {t('alias.conflictCancel')}
              </button>
              <button className="btn btn-primary" onClick={() => {
                invoke('add_visio_alias', { name: aliasConflictName, url: aliasConflictUrl }).catch(() => {})
                setAliasConflictName('')
                setAliasConflictUrl('')
              }}>
                {t('alias.conflictReplace')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

// -- Info Sidebar -----------------------------------------------------------

function InfoSidebar({
  meetUrl,
  onClose,
  roomId,
  accessLevel,
  roomDisplayName,
}: Readonly<{
  meetUrl: string
  onClose: () => void
  roomId?: string
  accessLevel?: string
  roomDisplayName?: string | null
}>) {
  const t = useT()
  const [copiedHttp, setCopiedHttp] = useState(false)
  const [copiedDeep, setCopiedDeep] = useState(false)
  const [roomAccesses, setRoomAccesses] = useState<any[]>([])
  const [memberSearch, setMemberSearch] = useState('')
  const [memberResults, setMemberResults] = useState<any[]>([])

  // Build share URL with room display name param if available
  const shareUrl = (() => {
    if (!roomDisplayName) return meetUrl
    const sep = meetUrl.includes('?') ? '&' : '?'
    return `${meetUrl}${sep}visio=${encodeURIComponent(roomDisplayName)}`
  })()

  // Normalize URL for display (strip scheme)
  const displayUrl = meetUrl.replace(/^https?:\/\//, '')
  const deepLinkBase = `visio://${displayUrl}`
  const deepLink = roomDisplayName
    ? `${deepLinkBase}?visio=${encodeURIComponent(roomDisplayName)}`
    : deepLinkBase

  // Fetch accesses on mount if roomId is provided
  useEffect(() => {
    if (!roomId) return
    const fetchAccesses = async () => {
      try {
        const results = await invoke<any[]>('list_accesses', { roomId })
        setRoomAccesses(results)
      } catch {
        /* ignore - not owner/admin */
      }
    }
    fetchAccesses()
  }, [roomId])

  // Member search effect
  useEffect(() => {
    if (memberSearch.length < 3) {
      setMemberResults([])
      return
    }
    const timer = setTimeout(async () => {
      try {
        const results = await invoke<any[]>('search_users', {
          query: memberSearch,
        })
        setMemberResults(
          results.filter(
            (u: any) => !roomAccesses.some((a: any) => a.user.id === u.id)
          )
        )
      } catch {
        setMemberResults([])
      }
    }, 300)
    return () => clearTimeout(timer)
  }, [memberSearch, roomAccesses])

  const handleCopyHttp = async () => {
    try {
      await navigator.clipboard.writeText(shareUrl)
      setCopiedHttp(true)
      setTimeout(() => setCopiedHttp(false), 2000)
    } catch {
      /* fallback: ignore */
    }
  }

  const handleCopyDeep = async () => {
    try {
      await navigator.clipboard.writeText(deepLink)
      setCopiedDeep(true)
      setTimeout(() => setCopiedDeep(false), 2000)
    } catch {
      /* fallback: ignore */
    }
  }

  return (
    <div className="info-sidebar">
      <div className="participants-header">
        <span>{t('info.title')}</span>
        <button
          className="chat-close"
          aria-label={t('action.close')}
          onClick={onClose}
        >
          <RiCloseLine size={20} />
        </button>
      </div>
      <div className="info-body">
        <div className="info-section">
          <div className="info-link-header">
            <RiGlobalLine size={16} />
            <span>{t('settings.incall.roomLink')}</span>
            <button
              className="info-copy-icon"
              onClick={handleCopyHttp}
              title={t('settings.incall.copied')}
            >
              {copiedHttp ? (
                <RiCheckLine size={16} />
              ) : (
                <RiFileCopyLine size={16} />
              )}
            </button>
          </div>
          <input
            className="info-link-input"
            readOnly
            value={shareUrl}
            onClick={(e) => (e.target as HTMLInputElement).select()}
          />
        </div>
        <div className="info-section">
          <div className="info-link-header">
            <RiSmartphoneLine size={16} />
            <span>{t('settings.incall.deepLink')}</span>
            <button
              className="info-copy-icon"
              onClick={handleCopyDeep}
              title={t('settings.incall.copied')}
            >
              {copiedDeep ? (
                <RiCheckLine size={16} />
              ) : (
                <RiFileCopyLine size={16} />
              )}
            </button>
          </div>
          <input
            className="info-link-input"
            readOnly
            value={deepLink}
            onClick={(e) => (e.target as HTMLInputElement).select()}
          />
        </div>
        {roomId && accessLevel === 'restricted' && (
          <div className="members-section">
            <h4 style={{ margin: '16px 0 8px' }}>{t('restricted.members')}</h4>
            <input
              type="text"
              className="info-link-input"
              placeholder={t('restricted.searchUsers')}
              value={memberSearch}
              onChange={(e) => setMemberSearch(e.target.value)}
            />
            {memberResults.length > 0 && (
              <div className="search-dropdown">
                {memberResults.map((user: any) => (
                  <button
                    key={user.id}
                    type="button"
                    className="search-result"
                    onClick={async () => {
                      try {
                        await invoke('add_access', { userId: user.id, roomId })
                        const updated = await invoke<any[]>('list_accesses', {
                          roomId,
                        })
                        setRoomAccesses(updated)
                      } catch {
                        /* ignore */
                      }
                      setMemberSearch('')
                      setMemberResults([])
                    }}
                  >
                    <span className="search-name">
                      {user.full_name || user.email}
                    </span>
                    <span className="search-email">{user.email}</span>
                  </button>
                ))}
              </div>
            )}
            {roomAccesses.map((access: any) => (
              <div key={access.id} className="member-row">
                <div className="member-info">
                  <span>{access.user.full_name || access.user.email}</span>
                  <span className="member-role">
                    {t(`restricted.${access.role}`)}
                  </span>
                </div>
                {access.role === 'member' && (
                  <button
                    className="btn btn-sm btn-danger"
                    onClick={async () => {
                      try {
                        await invoke('remove_access', { accessId: access.id })
                        setRoomAccesses((prev) =>
                          prev.filter((a: any) => a.id !== access.id)
                        )
                      } catch {
                        /* ignore */
                      }
                    }}
                  >
                    {t('restricted.remove')}
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

// -- Tools Sidebar ----------------------------------------------------------

function ToolsSidebar({ onClose }: Readonly<{ onClose: () => void }>) {
  const t = useT()
  const [subView, setSubView] = useState<'menu' | 'transcribe'>('menu')

  if (subView === 'transcribe') {
    return (
      <div className="info-sidebar">
        <div className="participants-header">
          <button
            className="chat-close"
            aria-label={t('action.back')}
            onClick={() => setSubView('menu')}
          >
            <RiArrowLeftSLine size={20} />
          </button>
          <span style={{ flex: 1 }}>{t('transcribe.title')}</span>
          <button
            className="chat-close"
            aria-label={t('action.close')}
            onClick={onClose}
          >
            <RiCloseLine size={20} />
          </button>
        </div>
        <div className="info-body transcribe-body">
          <h3 className="transcribe-heading">{t('transcribe.heading')}</h3>
          <p className="transcribe-sub">{t('transcribe.subheading')}</p>
          <div className="transcribe-features">
            <div className="transcribe-feature">
              <RiFileTextLine size={16} />
              <span>{t('transcribe.newDoc')}</span>
            </div>
            <div className="transcribe-feature">
              <RiMailLine size={16} />
              <span>{t('transcribe.emailSent')}</span>
            </div>
            <div className="transcribe-feature">
              <RiGlobalLine size={16} />
              <span>
                {t('transcribe.language')} : {t('transcribe.currentLanguage')}
              </span>
            </div>
          </div>
          <label className="transcribe-record-check">
            <input type="checkbox" />
            {t('transcribe.alsoRecord')}
          </label>
          <button className="btn btn-primary transcribe-start" disabled>
            {t('transcribe.start')}
          </button>
          <p className="transcribe-notice">{t('transcribe.comingSoon')}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="info-sidebar">
      <div className="participants-header">
        <span>{t('tools.title')}</span>
        <button
          className="chat-close"
          aria-label={t('action.close')}
          onClick={onClose}
        >
          <RiCloseLine size={20} />
        </button>
      </div>
      <div className="info-body">
        <p className="tools-subtitle">{t('tools.subtitle')}</p>
        <button className="tools-row" onClick={() => setSubView('transcribe')}>
          <span className="tools-row-icon">
            <RiFileTextLine size={20} />
          </span>
          <span className="tools-row-text">
            <span className="tools-row-label">{t('control.transcribe')}</span>
            <span className="tools-row-desc">{t('tools.transcribe.desc')}</span>
          </span>
          <RiArrowRightSLine size={18} />
        </button>
        <button className="tools-row" disabled>
          <span className="tools-row-icon">
            <RiRecordCircleLine size={20} />
          </span>
          <span className="tools-row-text">
            <span className="tools-row-label">{t('control.record')}</span>
            <span className="tools-row-desc">{t('tools.record.desc')}</span>
          </span>
          <RiArrowRightSLine size={18} />
        </button>
      </div>
    </div>
  )
}

// -- Waiting Screen ---------------------------------------------------------

function WaitingScreen({
  onCancel,
  t,
}: Readonly<{
  onCancel: () => void
  t: (k: string) => string
}>) {
  return (
    <div className="waiting-screen">
      <div className="waiting-content">
        <div className="waiting-spinner" />
        <h2>{t('lobby.waiting')}</h2>
        <p>{t('lobby.waitingDesc')}</p>
        <button className="btn btn-secondary" onClick={onCancel}>
          {t('lobby.cancel')}
        </button>
      </div>
    </div>
  )
}

// -- Source Picker Modal ----------------------------------------------------

function SourcePickerModal({
  sources,
  onSelect,
  onClose,
}: Readonly<{
  sources: ScreenSource[]
  onSelect: (sourceId: string) => void
  onClose: () => void
}>) {
  const t = useT()
  const monitors = sources.filter((s) => s.source_type === 'monitor')
  const windows = sources.filter((s) => s.source_type === 'window')

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="settings-modal source-picker"
        data-testid="screen-share-source-picker"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="settings-header">
          <span>{t('call.selectSource')}</span>
          <button onClick={onClose}>
            <RiCloseLine size={20} />
          </button>
        </div>
        <div className="source-grid">
          {monitors.length > 0 && (
            <>
              <h4 className="source-section-title">{t('call.monitors')}</h4>
              <div className="source-grid-items">
                {monitors.map((s, i) => (
                  <button
                    key={s.id}
                    className="source-card"
                    data-testid={`screen-share-source-${i}`}
                    onClick={() => onSelect(s.id)}
                  >
                    {s.thumbnail ? (
                      <img
                        src={s.thumbnail}
                        alt={s.name}
                        className="source-thumb"
                      />
                    ) : (
                      <div className="source-thumb source-thumb-placeholder">
                        <ScreenShareIcon size={32} />
                      </div>
                    )}
                    <span className="source-card-label">{s.name}</span>
                  </button>
                ))}
              </div>
            </>
          )}
          <h4 className="source-section-title">{t('call.windows')}</h4>
          {windows.length > 0 ? (
            <div className="source-grid-items">
              {windows.map((s, i) => (
                <button
                  key={s.id}
                  className="source-card"
                  data-testid={`screen-share-source-${monitors.length + i}`}
                  onClick={() => onSelect(s.id)}
                >
                  {s.thumbnail ? (
                    <img
                      src={s.thumbnail}
                      alt={s.name}
                      className="source-thumb"
                    />
                  ) : (
                    <div className="source-thumb source-thumb-placeholder">
                      <RiApps2Line size={32} />
                    </div>
                  )}
                  <span className="source-card-label">{s.name}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className="source-permission-hint">
              {t('call.screenPermissionHint')}
            </p>
          )}
        </div>
      </div>
    </div>
  )
}

// -- Call View --------------------------------------------------------------

function CallView({
  participants,
  localParticipant,
  micEnabled,
  camEnabled,
  videoFrames,
  messages,
  handRaisedMap,
  isHandRaised,
  unreadCount,
  showChat,
  onToggleMic,
  onToggleCam,
  onHangUp,
  onToggleHandRaise,
  onToggleChat,
  onSendChat,
  onToggleParticipants,
  showParticipants,
  onToggleInfo,
  showInfo,
  meetUrl,
  onToggleTranscription,
  showTranscription,
  onShowMicPicker,
  onShowCamPicker,
  showMicPicker,
  showCamPicker,
  audioInputs,
  audioOutputs,
  videoInputs,
  selectedAudioInput,
  selectedAudioOutput,
  selectedVideoInput,
  activeSpeakers,
  onSelectAudioInput,
  onSelectAudioOutput,
  onSelectVideoInput,
  waitingParticipants,
  setWaitingParticipants,
  roomId,
  accessLevel,
  roomDisplayName,
  bandwidthMode,
}: Readonly<{
  participants: Participant[]
  localParticipant: Participant | null
  micEnabled: boolean
  camEnabled: boolean
  videoFrames: Map<string, string>
  messages: ChatMessage[]
  handRaisedMap: Record<string, number>
  activeSpeakers: string[]
  isHandRaised: boolean
  unreadCount: number
  showChat: boolean
  onToggleMic: () => void
  onToggleCam: () => void
  onHangUp: () => void
  onToggleHandRaise: () => void
  onToggleChat: () => void
  onSendChat: (text: string) => void
  onToggleParticipants: () => void
  showParticipants: boolean
  onToggleInfo: () => void
  showInfo: boolean
  meetUrl: string
  onToggleTranscription: () => void
  showTranscription: boolean
  onShowMicPicker: () => void
  onShowCamPicker: () => void
  showMicPicker: boolean
  showCamPicker: boolean
  audioInputs: NativeAudioDevice[]
  audioOutputs: NativeAudioDevice[]
  videoInputs: NativeVideoDevice[]
  selectedAudioInput: string
  selectedAudioOutput: string
  selectedVideoInput: string
  onSelectAudioInput: (name: string) => void
  onSelectAudioOutput: (name: string) => void
  onSelectVideoInput: (uniqueId: string) => void
  waitingParticipants: Array<{ id: string; username: string }>
  setWaitingParticipants: React.Dispatch<
    React.SetStateAction<Array<{ id: string; username: string }>>
  >
  roomId?: string
  accessLevel?: string
  roomDisplayName?: string | null
  bandwidthMode?: string
}>) {
  const t = useT()
  const [focusedItem, setFocusedItem] = useState<FocusItem>(null)
  const userPinnedRef = useRef(false) // tracks whether user manually pinned a participant
  const [showFocusThumbnails, setShowFocusThumbnails] = useState(true)
  const [isScreenSharing, setIsScreenSharing] = useState(false)
  const [showSourcePicker, setShowSourcePicker] = useState(false)
  const [screenSources, setScreenSources] = useState<ScreenSource[]>([])
  const [chatInput, setChatInput] = useState('')
  const chatScrollRef = useRef<HTMLDivElement>(null)
  const [bgMode, setBgMode] = useState('off')
  const [showOverflow, setShowOverflow] = useState(false)
  const [showReactionPicker, setShowReactionPicker] = useState(false)
  const [reactions, setReactions] = useState<ReactionData[]>([])
  const reactionIdCounter = useRef(0)
  const [participantMenu, setParticipantMenu] = useState<string | null>(null)

  // Listen for reaction events
  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    listen<{ participantSid: string; participantName: string; emoji: string }>(
      'reaction-received',
      (event) => {
        const { participantSid, participantName, emoji } = event.payload
        const id = ++reactionIdCounter.current
        const reaction: ReactionData = {
          id,
          participantSid,
          participantName,
          emoji,
          timestamp: Date.now(),
        }
        setReactions((prev) => [...prev, reaction])
        // Auto-remove after 3 seconds
        setTimeout(() => {
          setReactions((prev) => prev.filter((r) => r.id !== id))
        }, 3000)
      }
    ).then((fn) => {
      unlisten = fn
    })
    return () => {
      unlisten?.()
    }
  }, [])

  // Auto-focus when a screen share track is subscribed
  useEffect(() => {
    const unsub = listen<{
      trackSid: string
      participantSid: string
      source: string
    }>('track-subscribed', (event) => {
      if (event.payload.source === 'screen_share') {
        setFocusedItem({
          participantSid: event.payload.participantSid,
          source: 'screen_share',
        })
      }
    })
    return () => {
      unsub.then((f) => f())
    }
  }, [])

  // Auto-unfocus when focused screen share ends
  useEffect(() => {
    if (focusedItem?.source === 'screen_share') {
      // Check both remote participants and local participant
      const isLocal =
        localParticipant && focusedItem.participantSid === localParticipant.sid
      if (isLocal) {
        if (!isScreenSharing) {
          userPinnedRef.current = false
          setFocusedItem(null)
        }
      } else {
        const p = participants.find((p) => p.sid === focusedItem.participantSid)
        if (!p?.has_screen_share) {
          userPinnedRef.current = false
          setFocusedItem(null)
        }
      }
    }
  }, [participants, focusedItem, localParticipant, isScreenSharing])

  // Grid is the default layout — no auto-focus on active speaker.
  // Users must explicitly click a participant to pin them.

  const handleSendReaction = async (emojiId: string) => {
    try {
      await invoke('send_reaction', { emoji: emojiId })
      // Show reaction locally (the echo from the server is filtered out)
      const id = ++reactionIdCounter.current
      setReactions((prev) => [
        ...prev,
        {
          id,
          participantSid: localParticipant?.sid ?? '',
          participantName: localParticipant?.name ?? '',
          emoji: emojiId,
          timestamp: Date.now(),
        },
      ])
    } catch (e) {
      console.error('send_reaction error:', e)
    }
    setShowReactionPicker(false)
    setShowOverflow(false)
  }

  // Load current background mode on mount
  useEffect(() => {
    invoke<string>('get_background_mode')
      .then(setBgMode)
      .catch(() => {})
  }, [])

  const handleBgMode = async (mode: string) => {
    try {
      if (mode.startsWith('image:')) {
        const id = Number.parseInt(mode.slice(6), 10)
        const path = await resolveResource(`backgrounds/${id}.jpg`)
        await invoke('load_background_image', { id, jpegPath: path })
      }
      await invoke('set_background_mode', { mode })
      setBgMode(mode)
    } catch (e) {
      console.error('set_background_mode error:', e)
    }
  }

  // Close overflow/reaction picker when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Element
      if (!target.closest('.overflow-menu, .reaction-picker, .control-btn')) {
        setShowOverflow(false)
        setShowReactionPicker(false)
      }
      if (!target.closest('.participant-menu-wrapper')) {
        setParticipantMenu(null)
      }
    }
    document.addEventListener('click', handleClickOutside)
    return () => document.removeEventListener('click', handleClickOutside)
  }, [])

  useEffect(() => {
    if (chatScrollRef.current) {
      chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight
    }
  }, [messages.length])

  const sendMessage = () => {
    const text = chatInput.trim()
    if (!text) return
    setChatInput('')
    onSendChat(text)
  }

  // Build allParticipants with local participant first
  const allParticipants: Participant[] = []
  if (localParticipant) {
    // Override local participant's name to show "You" label, and sync mute/video state
    allParticipants.push({
      ...localParticipant,
      name: localParticipant.name
        ? `${localParticipant.name} (${t('call.you')})`
        : t('call.you'),
      is_muted: !micEnabled,
      has_video: camEnabled,
      video_track_sid: camEnabled ? 'local-camera' : null,
      has_screen_share: isScreenSharing,
      screen_share_track_sid: isScreenSharing ? 'local-screen' : null,
    })
  }
  allParticipants.push(
    ...participants.filter(
      (p) => !localParticipant || p.sid !== localParticipant.sid
    )
  )
  const displayItems = buildDisplayItems(allParticipants, t)
  const focusedDisplayItem = focusedItem
    ? displayItems.find(
        (d) =>
          d.participant.sid === focusedItem.participantSid &&
          d.source === focusedItem.source
      )
    : null
  const thumbnailItems = focusedDisplayItem
    ? displayItems.filter((d) => d.key !== focusedDisplayItem.key)
    : []
  // Compute grid layout: choose columns so all tiles are uniform
  const gridCount = displayItems.length
  let gridCols = 5
  if (gridCount <= 1) gridCols = 1
  else if (gridCount <= 4) gridCols = 2
  else if (gridCount <= 9) gridCols = 3
  else if (gridCount <= 16) gridCols = 4
  const gridRows = Math.ceil(gridCount / gridCols)
  const gridStyle: React.CSSProperties =
    gridCount > 0
      ? {
          gridTemplateColumns: `repeat(${gridCols}, 1fr)`,
          gridTemplateRows: `repeat(${gridRows}, 1fr)`,
        }
      : {}

  return (
    <div id="call" className="section active">
      {/* Lobby waiting banner — persistent while participants are waiting */}
      {waitingParticipants.length > 0 &&
        (() => {
          const first = waitingParticipants[0]
          const parts = t('lobby.joinRequest').split('{{name}}')
          const suffix =
            waitingParticipants.length > 1
              ? ` (+${waitingParticipants.length - 1})`
              : ''
          return (
            <div className="lobby-notification">
              <span className="lobby-notification-text">
                {parts[0]}
                <strong>{first.username}</strong>
                {parts[1]}
                {suffix}
              </span>
              <div className="lobby-notification-actions">
                <button
                  className="btn-admit"
                  onClick={async () => {
                    try {
                      await invoke('admit_participant', {
                        participantId: first.id,
                      })
                      setWaitingParticipants((prev) =>
                        prev.filter((x) => x.id !== first.id)
                      )
                    } catch (e) {
                      console.error('admit error:', e)
                    }
                  }}
                >
                  {t('lobby.admit')}
                </button>
                <button
                  className="btn-view"
                  onClick={() => {
                    if (!showParticipants) onToggleParticipants()
                  }}
                >
                  {t('lobby.view')}
                </button>
              </div>
            </div>
          )
        })()}
      <div className="call-body">
        {/* Main video area */}
        <div
          className="call-content"
          data-testid={`layout-mode:${focusedDisplayItem ? 'FOCUS' : 'GRID'}`}
        >
          {focusedDisplayItem ? (
            <div className="focus-layout">
              <div
                className="focus-main"
                data-testid={`main-tile:${focusedDisplayItem.participant.sid}`}
              >
                {userPinnedRef.current && (
                  <span
                    className="pin-indicator"
                    data-testid={`pin-indicator:${focusedDisplayItem.participant.sid}`}
                    aria-hidden="true"
                  />
                )}
                <ParticipantTile
                  participant={focusedDisplayItem.participant}
                  videoFrames={videoFrames}
                  isActiveSpeaker={activeSpeakers.includes(
                    focusedDisplayItem.participant.sid
                  )}
                  handRaisePosition={
                    handRaisedMap[focusedDisplayItem.participant.sid]
                  }
                  displayItem={focusedDisplayItem}
                  bandwidthMode={bandwidthMode}
                />
                <div className="focus-toolbar">
                  <button
                    className="focus-toolbar-btn"
                    onClick={() => setShowFocusThumbnails((v) => !v)}
                    title={
                      showFocusThumbnails
                        ? t('call.focus.hideThumbnails')
                        : t('call.focus.showThumbnails')
                    }
                  >
                    {showFocusThumbnails ? (
                      <RiFullscreenLine size={18} />
                    ) : (
                      <RiFullscreenExitLine size={18} />
                    )}
                  </button>
                  <button
                    className="focus-toolbar-btn"
                    onClick={() => {
                      setFocusedItem(null)
                      userPinnedRef.current = false
                      setShowFocusThumbnails(true)
                    }}
                    title={t('call.focus.backToGrid')}
                  >
                    <RiCloseLine size={18} />
                  </button>
                </div>
              </div>
              {showFocusThumbnails && thumbnailItems.length > 0 && (
                <div className="focus-thumbnails">
                  {thumbnailItems.map((d, index) => (
                    <button
                      key={d.key}
                      type="button"
                      className="tile"
                      data-testid={`secondary-tile-${index}:${d.participant.sid}`}
                      onClick={() => {
                        userPinnedRef.current = true
                        setFocusedItem({
                          participantSid: d.participant.sid,
                          source: d.source,
                        })
                      }}
                    >
                      <ParticipantTile
                        participant={d.participant}
                        videoFrames={videoFrames}
                        isActiveSpeaker={activeSpeakers.includes(
                          d.participant.sid
                        )}
                        handRaisePosition={handRaisedMap[d.participant.sid]}
                        displayItem={d}
                        bandwidthMode={bandwidthMode}
                      />
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div
              className={`video-grid${gridCount === 0 ? ' video-grid-0' : ''}`}
              style={gridStyle}
              data-testid="call-participant-grid"
            >
              {displayItems.length === 0 ? (
                <div className="empty-state">{t('call.noParticipants')}</div>
              ) : (
                displayItems.map((d, index) => (
                  <button
                    key={d.key}
                    type="button"
                    data-testid={`grid-tile-${index}:${d.participant.sid}`}
                    onClick={() => {
                      userPinnedRef.current = true
                      setFocusedItem({
                        participantSid: d.participant.sid,
                        source: d.source,
                      })
                    }}
                  >
                    <ParticipantTile
                      participant={d.participant}
                      videoFrames={videoFrames}
                      isActiveSpeaker={activeSpeakers.includes(
                        d.participant.sid
                      )}
                      handRaisePosition={handRaisedMap[d.participant.sid]}
                      displayItem={d}
                      bandwidthMode={bandwidthMode}
                      onExpand={
                        d.isScreenShare
                          ? () => {
                              userPinnedRef.current = true
                              setFocusedItem({
                                participantSid: d.participant.sid,
                                source: d.source,
                              })
                            }
                          : undefined
                      }
                    />
                  </button>
                ))
              )}
            </div>
          )}
        </div>

        {/* Chat sidebar */}
        {showChat && (
          <div className="chat-sidebar" data-testid="call-chat-sidebar">
            <div className="chat-header">
              <span>{t('chat')}</span>
              <button
                className="chat-close"
                aria-label={t('action.close')}
                data-testid="chat-close-button"
                onClick={onToggleChat}
              >
                <RiCloseLine size={20} />
              </button>
            </div>
            <div
              className="chat-messages"
              ref={chatScrollRef}
              data-testid="chat-message-list"
            >
              {messages.length === 0 ? (
                <div className="chat-empty" data-testid="chat-empty">
                  {t('chat.noMessages')}
                </div>
              ) : (
                messages.map((m, i) => {
                  const isOwn =
                    localParticipant && m.sender_sid === localParticipant.sid
                  const showName =
                    !isOwn &&
                    (i === 0 || messages[i - 1].sender_sid !== m.sender_sid)
                  return (
                    <div
                      key={m.id}
                      className={`chat-bubble ${isOwn ? 'chat-bubble-own' : ''}`}
                      data-testid={`chat-bubble-${i}`}
                    >
                      {showName && (
                        <div className="chat-sender">
                          {m.sender_name || t('unknown')}
                        </div>
                      )}
                      <div className="chat-text">
                        <AutoLinkText text={m.text} />
                      </div>
                      <div className="chat-time">
                        {formatTime(m.timestamp_ms)}
                      </div>
                    </div>
                  )
                })
              )}
            </div>
            <div className="chat-input-bar">
              <input
                className="chat-input"
                data-testid="chat-message-input"
                value={chatInput}
                onChange={(e) => setChatInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && sendMessage()}
                maxLength={2000}
                placeholder={t('chat.placeholder')}
              />
              <button
                className="chat-send"
                data-testid="chat-send-button"
                onClick={sendMessage}
                disabled={!chatInput.trim()}
              >
                <RiSendPlane2Fill size={18} />
              </button>
            </div>
          </div>
        )}

        {/* Participants sidebar */}
        {showParticipants && (
          <div className="participants-sidebar">
            <div className="participants-header">
              <span>
                {t('control.participants')}{' '}
                <span className="participants-count">
                  ({allParticipants.length})
                </span>
              </span>
              <button
                className="chat-close"
                aria-label={t('action.close')}
                onClick={onToggleParticipants}
              >
                <RiCloseLine size={20} />
              </button>
            </div>
            <div className="participants-list">
              {waitingParticipants.length > 0 && (
                <div className="lobby-section">
                  <div className="lobby-header">
                    <h4>
                      {t('lobby.waitingParticipants')} (
                      {waitingParticipants.length})
                    </h4>
                    <button
                      className="btn btn-sm"
                      onClick={async () => {
                        for (const p of waitingParticipants) {
                          await invoke('admit_participant', {
                            participantId: p.id,
                          })
                        }
                        setWaitingParticipants([])
                      }}
                    >
                      {t('lobby.admitAll')}
                    </button>
                  </div>
                  {waitingParticipants.map((p) => (
                    <div key={p.id} className="lobby-participant">
                      <span>{p.username}</span>
                      <div className="lobby-actions">
                        <button
                          className="btn btn-sm btn-primary"
                          onClick={async () => {
                            await invoke('admit_participant', {
                              participantId: p.id,
                            })
                            setWaitingParticipants((prev) =>
                              prev.filter((x) => x.id !== p.id)
                            )
                          }}
                        >
                          {t('lobby.admit')}
                        </button>
                        <button
                          className="btn btn-sm btn-danger"
                          onClick={async () => {
                            await invoke('deny_participant', {
                              participantId: p.id,
                            })
                            setWaitingParticipants((prev) =>
                              prev.filter((x) => x.id !== p.id)
                            )
                          }}
                        >
                          {t('lobby.deny')}
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {allParticipants.map((p) => {
                const name = p.name || p.identity || t('unknown')
                const isLocal =
                  localParticipant && p.sid === localParticipant.sid
                const isLocalAdmin = localParticipant?.is_admin
                const isPinned = focusedItem?.participantSid === p.sid
                const menuOpen = participantMenu === p.sid
                return (
                  <div key={p.sid} className="participant-row">
                    <div
                      className="participant-avatar-sm"
                      style={{ background: `hsl(${getHue(name)}, 50%, 35%)` }}
                    >
                      {getInitials(name)}
                    </div>
                    <div className="participant-info">
                      <div className="participant-display-name">{name}</div>
                      {isLocal && (
                        <div className="participant-you-label">
                          {t('call.you')}
                        </div>
                      )}
                    </div>
                    <div className="participant-icons">
                      {p.is_muted ? (
                        <RiMicOffFill size={14} className="muted-icon" />
                      ) : activeSpeakers.includes(p.sid) ? (
                        <RiMicLine size={14} className="speaking-icon" />
                      ) : null}
                      {handRaisedMap[p.sid] > 0 && (
                        <RiHand
                          size={14}
                          style={{ color: 'var(--hand-raise)' }}
                        />
                      )}
                      <ConnectionQualityBars quality={p.connection_quality} />
                      {!isLocal && (
                        <div className="participant-menu-wrapper">
                          <button
                            className="participant-menu-btn"
                            onClick={(e) => {
                              e.stopPropagation()
                              setParticipantMenu(menuOpen ? null : p.sid)
                            }}
                          >
                            <RiMore2Fill size={16} />
                          </button>
                          {menuOpen && (
                            <div
                              className="participant-context-menu"
                              onClick={() => setParticipantMenu(null)}
                            >
                              <button
                                className="context-menu-item"
                                onClick={() => {
                                  if (isPinned) {
                                    userPinnedRef.current = false
                                    setFocusedItem(null)
                                  } else {
                                    userPinnedRef.current = true
                                    setFocusedItem({
                                      participantSid: p.sid,
                                      source: 'camera',
                                    })
                                  }
                                }}
                              >
                                {isPinned ? (
                                  <RiUnpinFill size={16} />
                                ) : (
                                  <RiPushpinLine size={16} />
                                )}
                                <span>
                                  {isPinned
                                    ? t('participant.unpin')
                                    : t('participant.pin')}
                                </span>
                              </button>
                              {isLocalAdmin && !p.is_muted && (
                                <button
                                  className="context-menu-item"
                                  onClick={async () => {
                                    await invoke('mute_participant', {
                                      identity: p.identity,
                                    })
                                  }}
                                >
                                  <RiVolumeMuteLine size={16} />
                                  <span>{t('participant.mute')}</span>
                                </button>
                              )}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        )}

        {/* Info sidebar */}
        {showInfo && !showTranscription && (
          <InfoSidebar
            meetUrl={meetUrl}
            onClose={onToggleInfo}
            roomId={roomId}
            accessLevel={accessLevel}
            roomDisplayName={roomDisplayName}
          />
        )}

        {/* Tools sidebar */}
        {showTranscription && <ToolsSidebar onClose={onToggleTranscription} />}
      </div>

      {/* Reaction overlay */}
      {reactions.length > 0 && (
        <div className="reaction-overlay">
          {reactions.map((r) => {
            const emojiChar =
              REACTION_EMOJIS.find(([id]) => id === r.emoji)?.[1] ?? r.emoji
            return (
              <div key={r.id} className="floating-reaction">
                <span className="floating-reaction-emoji">{emojiChar}</span>
                <span className="floating-reaction-name">
                  {r.participantName}
                </span>
              </div>
            )
          })}
        </div>
      )}

      {/* Overflow menu */}
      {showOverflow && (
        <div className="overflow-menu">
          <button
            className={`overflow-item ${isHandRaised ? 'overflow-item-active' : ''}`}
            onClick={() => {
              onToggleHandRaise()
              setShowOverflow(false)
            }}
            data-testid="call-hand-raise-button"
          >
            <RiHand size={20} />
            <span>
              {isHandRaised ? t('control.lowerHand') : t('control.raiseHand')}
            </span>
          </button>
          <button
            className="overflow-item"
            onClick={() => {
              setShowReactionPicker(!showReactionPicker)
              setShowOverflow(false)
            }}
          >
            <RiEmotionLine size={20} />
            <span>{t('control.reaction') ?? 'Reaction'}</span>
          </button>
          <button
            className={`overflow-item ${showTranscription ? 'overflow-item-active' : ''}`}
            onClick={() => {
              onToggleTranscription()
              setShowOverflow(false)
            }}
          >
            <RiApps2Line size={20} />
            <span>{t('control.tools')}</span>
          </button>
          <button
            className={`overflow-item ${showInfo ? 'overflow-item-active' : ''}`}
            onClick={() => {
              onToggleInfo()
              setShowOverflow(false)
            }}
          >
            <RiInformationLine size={20} />
            <span>{t('control.info')}</span>
          </button>
          <button
            className="overflow-item"
            onClick={() => {
              setShowOverflow(false)
            }}
            title={t('control.settings') ?? 'Settings'}
          >
            <RiSettings3Line size={20} />
            <span>{t('control.settings') ?? 'Settings'}</span>
          </button>
        </div>
      )}

      {/* Reaction picker */}
      {showReactionPicker && (
        <div className="reaction-picker">
          {REACTION_EMOJIS.map(([id, char]) => (
            <button
              key={id}
              className="reaction-picker-btn"
              onClick={() => handleSendReaction(id)}
              title={id}
            >
              {char}
            </button>
          ))}
        </div>
      )}

      {/* Control bar */}
      <div className="control-bar">
        {/* Mic group */}
        <div className="control-group">
          <button
            className={`control-btn ${micEnabled ? '' : 'control-btn-off'}`}
            onClick={onToggleMic}
            title={micEnabled ? t('control.mute') : t('control.unmute')}
            style={{ borderRadius: '8px 0 0 8px' }}
            data-testid="call-mic-button"
          >
            {micEnabled ? <RiMicLine size={20} /> : <RiMicOffLine size={20} />}
          </button>
          <button
            className={`control-btn control-chevron ${micEnabled ? '' : 'control-btn-off'}`}
            onClick={onShowMicPicker}
            title={t('control.audioDevices')}
            style={{ borderRadius: '0 8px 8px 0' }}
            data-testid="call-mic-chevron"
          >
            <RiArrowUpSLine size={16} />
          </button>
        </div>

        {/* Camera group */}
        <div className="control-group">
          <button
            className={`control-btn ${camEnabled ? '' : 'control-btn-off'}`}
            onClick={onToggleCam}
            title={camEnabled ? t('control.camOff') : t('control.camOn')}
            style={{ borderRadius: '8px 0 0 8px' }}
            data-testid="call-camera-button"
          >
            {camEnabled ? (
              <RiVideoOnLine size={20} />
            ) : (
              <RiVideoOffLine size={20} />
            )}
          </button>
          <button
            className={`control-btn control-chevron ${camEnabled ? '' : 'control-btn-off'}`}
            onClick={onShowCamPicker}
            title={t('control.camDevices')}
            style={{ borderRadius: '0 8px 8px 0' }}
            data-testid="call-camera-chevron"
          >
            <RiArrowUpSLine size={16} />
          </button>
        </div>

        {/* Screen share */}
        <button
          className={`control-btn ${isScreenSharing ? 'control-btn-active-danger' : ''}`}
          onClick={async () => {
            if (isScreenSharing) {
              try {
                await invoke('stop_screen_share')
                setIsScreenSharing(false)
              } catch (e) {
                console.error('Failed to stop screen share:', e)
              }
            } else {
              try {
                const sources = await invoke<ScreenSource[]>(
                  'list_screen_sources'
                )
                setScreenSources(sources)
                setShowSourcePicker(true)
              } catch (e) {
                console.error('Failed to list screen sources:', e)
              }
            }
          }}
          title={isScreenSharing ? t('call.stopShare') : t('call.startShare')}
          data-testid="call-screen-share-button"
        >
          <ScreenShareIcon size={20} />
        </button>

        {/* Participants */}
        <button
          className={`control-btn ${showParticipants ? 'control-btn-hand' : ''}`}
          onClick={onToggleParticipants}
          title={t('control.participants')}
          data-testid="call-participants-button"
        >
          <RiGroupLine size={20} />
          <span
            className="unread-badge"
            style={{ background: 'var(--accent)' }}
          >
            {allParticipants.length}
          </span>
        </button>

        {/* Chat */}
        <button
          className={`control-btn ${showChat ? 'control-btn-hand' : ''}`}
          onClick={onToggleChat}
          title={t('chat')}
          data-testid="call-chat-button"
        >
          <RiChat1Line size={20} />
          {unreadCount > 0 && (
            <span className="unread-badge" data-testid="chat-unread-badge">
              {unreadCount > 9 ? '9+' : unreadCount}
            </span>
          )}
        </button>

        {/* More (overflow) */}
        <button
          className={`control-btn ${showOverflow ? 'control-btn-hand' : ''}`}
          onClick={() => {
            setShowOverflow(!showOverflow)
            setShowReactionPicker(false)
          }}
          title={t('control.more') ?? 'More'}
        >
          <RiMore2Fill size={20} />
        </button>

        {/* Hangup */}
        <button
          className="control-btn control-btn-hangup"
          onClick={onHangUp}
          title={t('control.leave')}
          data-testid="call-hangup-button"
        >
          <RiPhoneFill size={20} />
        </button>
      </div>

      {/* Mic device picker */}
      {showMicPicker && (
        <div className="device-picker" data-testid="device-picker-audio">
          <div className="device-section">
            <div className="device-section-title">{t('device.microphone')}</div>
            {audioInputs.map((d, i) => (
              <label
                key={d.name}
                className="device-option"
                data-testid={`device-option-input-${i}`}
              >
                <input
                  type="radio"
                  name="audioInput"
                  checked={selectedAudioInput === d.name}
                  onChange={() => onSelectAudioInput(d.name)}
                />
                {d.name}
                {d.is_default && ' \u2605'}
              </label>
            ))}
            {audioInputs.length === 0 && (
              <div
                style={{
                  fontSize: '0.8rem',
                  color: '#929292',
                  padding: '4px 8px',
                }}
              >
                {t('device.noMic')}
              </div>
            )}
          </div>
          <div className="device-section">
            <div className="device-section-title">{t('device.speaker')}</div>
            {audioOutputs.map((d, i) => (
              <label
                key={d.name}
                className="device-option"
                data-testid={`device-option-output-${i}`}
              >
                <input
                  type="radio"
                  name="audioOutput"
                  checked={selectedAudioOutput === d.name}
                  onChange={() => onSelectAudioOutput(d.name)}
                />
                {d.name}
                {d.is_default && ' \u2605'}
              </label>
            ))}
            {audioOutputs.length === 0 && (
              <div
                style={{
                  fontSize: '0.8rem',
                  color: '#929292',
                  padding: '4px 8px',
                }}
              >
                {t('device.noSpeaker')}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Camera device picker */}
      {showCamPicker && (
        <div
          className="device-picker"
          data-testid="device-picker-video"
          style={{ minWidth: 300 }}
        >
          <div className="device-section">
            <div className="device-section-title">{t('device.camera')}</div>
            {videoInputs.map((d, i) => (
              <label
                key={d.unique_id}
                className="device-option"
                data-testid={`device-option-camera-${i}`}
              >
                <input
                  type="radio"
                  name="videoInput"
                  checked={selectedVideoInput === d.unique_id}
                  onChange={() => onSelectVideoInput(d.unique_id)}
                />
                {d.name}
                {d.is_default && ' \u2605'}
              </label>
            ))}
            {videoInputs.length === 0 && (
              <div
                style={{
                  fontSize: '0.8rem',
                  color: '#929292',
                  padding: '4px 8px',
                }}
              >
                {t('device.noCamera')}
              </div>
            )}
          </div>
          <div className="device-section">
            <div className="device-section-title">
              {t('settings.incall.background')}
            </div>
            <div className="bg-mode-buttons">
              <button
                className={`bg-mode-btn ${bgMode === 'off' ? 'bg-mode-btn-active' : ''}`}
                onClick={() => handleBgMode('off')}
              >
                {t('settings.incall.bgOff')}
              </button>
              <button
                className={`bg-mode-btn ${bgMode === 'blur' ? 'bg-mode-btn-active' : ''}`}
                onClick={() => handleBgMode('blur')}
              >
                {t('settings.incall.bgBlur')}
              </button>
            </div>
            <div className="bg-image-grid">
              {[1, 2, 3, 4, 5, 6, 7, 8].map((id) => (
                <button
                  key={id}
                  className={`bg-image-thumb ${bgMode === 'image:' + id ? 'bg-image-thumb-active' : ''}`}
                  onClick={() => handleBgMode('image:' + id)}
                >
                  <img
                    src={`/backgrounds/thumbnails/${id}.jpg`}
                    alt={`Background ${id}`}
                    draggable={false}
                  />
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
      {showSourcePicker && (
        <SourcePickerModal
          sources={screenSources}
          onSelect={async (sourceId) => {
            setShowSourcePicker(false)
            try {
              await invoke('start_screen_share', { sourceId })
              setIsScreenSharing(true)
            } catch (e) {
              console.error('Failed to start screen share:', e)
            }
          }}
          onClose={() => setShowSourcePicker(false)}
        />
      )}
    </div>
  )
}

// -- Settings View ----------------------------------------------------------

function SettingsView({
  onClose,
  onLanguageChange,
  onThemeChange,
  onDisplayNameChange,
  initialDisplayName,
}: Readonly<{
  onClose: () => void
  onLanguageChange: (lang: string) => void
  onThemeChange: (theme: string) => void
  onDisplayNameChange: (name: string) => void
  initialDisplayName: string
}>) {
  const t = useT()
  const [form, setForm] = useState({
    displayName: initialDisplayName,
    language: 'fr',
    micOnJoin: true,
    cameraOnJoin: false,
    theme: 'light',
    adaptiveModeEnabled: false,
  })
  const [meetInstances, setMeetInstances] = useState<string[]>([
    'meet.numerique.gouv.fr',
  ])
  const [newInstance, setNewInstance] = useState('')
  const [calendarUrl, setCalendarUrl] = useState('')
  const [calendarRefreshInterval, setCalendarRefreshInterval] =
    useState('Minutes15')

  const addInstance = () => {
    const val = newInstance.trim().toLowerCase()
    if (val && !meetInstances.includes(val)) {
      const next = [...meetInstances, val]
      setMeetInstances(next)
      invoke('set_meet_instances', { instances: next })
      setNewInstance('')
    }
  }

  useEffect(() => {
    invoke<Settings>('get_settings')
      .then((s) => {
        setForm((prev) => ({
          ...prev,
          language: s.language || 'fr',
          micOnJoin: s.mic_enabled_on_join ?? true,
          cameraOnJoin: s.camera_enabled_on_join ?? false,
          theme: s.theme || 'light',
          adaptiveModeEnabled: s.adaptive_mode_enabled ?? false,
        }))
      })
      .catch(() => {})
    invoke<string[]>('get_meet_instances')
      .then(setMeetInstances)
      .catch(() => {})
    invoke<string | null>('get_calendar_url')
      .then((url) => setCalendarUrl(url ?? ''))
      .catch(() => {})
    invoke<string>('get_calendar_refresh_interval')
      .then((interval) => setCalendarRefreshInterval(interval))
      .catch(() => {})
  }, [])

  const [saveStatus, setSaveStatus] = useState<string | null>(null)

  const save = async () => {
    await invoke('set_display_name', { name: form.displayName || null })
    await invoke('set_mic_enabled_on_join', { enabled: form.micOnJoin })
    await invoke('set_camera_enabled_on_join', { enabled: form.cameraOnJoin })
    await invoke('set_adaptive_mode_enabled', {
      enabled: form.adaptiveModeEnabled,
    })
    await invoke('set_calendar_url', { url: calendarUrl.trim() || null })
    await invoke('set_calendar_refresh_interval', {
      interval: calendarRefreshInterval,
    })
    if (calendarUrl.trim()) {
      try {
        await invoke('refresh_calendar_now')
      } catch {
        // calendar refresh is best-effort on save
      }
    }
    setSaveStatus(t('settings.saved'))
    onDisplayNameChange(form.displayName)
    setTimeout(() => onClose(), 800)
  }

  return (
    <div className="settings-page">
      <div className="settings-page-header">
        <button className="settings-back-btn" onClick={onClose}>
          <RiArrowLeftSLine size={22} />
        </button>
        <span>{t('settings')}</span>
      </div>
      <div className="settings-page-body">
        <div className="settings-section">
          <label className="settings-label">{t('settings.displayName')}</label>
          <input
            className="settings-input"
            data-testid="settings-display-name-input"
            value={form.displayName}
            onChange={(e) => setForm({ ...form, displayName: e.target.value })}
          />
        </div>
        <div className="settings-section">
          <label className="settings-label">{t('settings.language')}</label>
          <select
            value={form.language}
            data-testid="settings-language-select"
            onChange={(e) => {
              const lang = e.target.value
              setForm({ ...form, language: lang })
              invoke('set_language', { lang: lang || null })
              onLanguageChange(lang)
            }}
          >
            {SUPPORTED_LANGS.map((code) => (
              <option
                key={code}
                value={code}
                data-testid={`settings-language-${code}`}
              >
                {translations[code]['lang.' + code]}
              </option>
            ))}
          </select>
        </div>
        <div className="settings-section">
          <label className="settings-label">{t('settings.theme')}</label>
          <select
            value={form.theme}
            onChange={(e) => {
              const theme = e.target.value
              setForm({ ...form, theme })
              invoke('set_theme', { theme })
              onThemeChange(theme)
            }}
          >
            <option value="light">{t('settings.theme.light')}</option>
            <option value="dark">{t('settings.theme.dark')}</option>
          </select>
        </div>
        <div className="settings-section">
          <label className="settings-label">{t('settings.micOnJoin')}</label>
          <input
            type="checkbox"
            checked={form.micOnJoin}
            onChange={(e) => setForm({ ...form, micOnJoin: e.target.checked })}
          />
        </div>
        <div className="settings-section">
          <label className="settings-label">{t('settings.camOnJoin')}</label>
          <input
            type="checkbox"
            checked={form.cameraOnJoin}
            onChange={(e) =>
              setForm({ ...form, cameraOnJoin: e.target.checked })
            }
          />
        </div>
        <div className="settings-section">
          <label className="settings-label">{t('settings.adaptiveMode')}</label>
          <input
            type="checkbox"
            checked={form.adaptiveModeEnabled}
            onChange={(e) =>
              setForm({ ...form, adaptiveModeEnabled: e.target.checked })
            }
          />
        </div>
        <div className="settings-section settings-section-col">
          <label className="settings-label">
            {t('settings.meetInstances')}
          </label>
          {meetInstances.map((inst) => (
            <div key={inst} className="instance-row">
              <span>{inst}</span>
              <button
                className="btn-icon"
                aria-label={t('action.remove')}
                onClick={() => {
                  const next = meetInstances.filter((x) => x !== inst)
                  setMeetInstances(next)
                  invoke('set_meet_instances', { instances: next })
                }}
              >
                <RiCloseLine size={16} />
              </button>
            </div>
          ))}
          <div className="instance-add-row">
            <input
              id="newInstance"
              type="text"
              placeholder={t('settings.instancePlaceholder')}
              value={newInstance}
              onChange={(e) => setNewInstance(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') addInstance()
              }}
            />
            <button
              className="btn-icon"
              aria-label={t('action.add')}
              onClick={addInstance}
              disabled={!newInstance.trim()}
            >
              <RiAddLine size={16} />
            </button>
          </div>
        </div>
        <div className="settings-section settings-section-col">
          <label className="settings-label">{t('settings.calendarUrl')}</label>
          <input
            className="settings-input"
            type="url"
            placeholder="https://cal.example.com/feed.ics"
            value={calendarUrl}
            onChange={(e) => setCalendarUrl(e.target.value)}
          />
          <span className="settings-hint">
            {t('settings.calendarUrl.hint')}
          </span>
        </div>
        <div className="settings-section">
          <label className="settings-label">
            {t('settings.calendarRefresh')}
          </label>
          <select
            value={calendarRefreshInterval}
            onChange={(e) => setCalendarRefreshInterval(e.target.value)}
          >
            <option value="Minutes5">
              {t('settings.calendarRefresh.5min')}
            </option>
            <option value="Minutes15">
              {t('settings.calendarRefresh.15min')}
            </option>
            <option value="Hour1">{t('settings.calendarRefresh.1h')}</option>
            <option value="Hours4">{t('settings.calendarRefresh.4h')}</option>
            <option value="Manual">
              {t('settings.calendarRefresh.manual')}
            </option>
          </select>
        </div>
      </div>
      <div className="settings-page-footer">
        <button
          className="settings-clear-history"
          onClick={async () => {
            await invoke('clear_visio_history')
            setSaveStatus(t('settings.historyCleared'))
            setTimeout(() => setSaveStatus(null), 2000)
          }}
        >
          {t('settings.clearHistory')}
        </button>
        {saveStatus && (
          <span className="settings-save-status">{saveStatus}</span>
        )}
        <button className="settings-save" onClick={save}>
          {t('settings.save')}
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Shared hook: audio device fallback on Bluetooth connect/disconnect
// ---------------------------------------------------------------------------

// resolveAudioFallback, handleAudioDevicesChanged, and
// useAudioDeviceFallback have been moved into useDeviceEnumeration.ts

// ---------------------------------------------------------------------------
// Pre-Join Screen
// ---------------------------------------------------------------------------

// NativeAudioDevice and NativeVideoDevice aliases removed — using
// NativeAudioDevice and NativeVideoDevice from useDeviceEnumeration.ts

/** Save user preferences (display name, mic/camera state, audio mode) before joining. */
async function savePreJoinPreferences(
  displayName: string | null,
  isMicOn: boolean,
  isCameraOn: boolean,
  audioMode: string
) {
  await invoke('set_display_name', { name: displayName }).catch(() => {})
  await invoke('set_mic_enabled_on_join', { enabled: isMicOn }).catch(() => {})
  await invoke('set_camera_enabled_on_join', { enabled: isCameraOn }).catch(
    () => {}
  )
  await invoke('set_audio_mode', { mode: audioMode }).catch(() => {})
}

/** Stop camera and mic previews before transitioning to the call screen. */
async function stopPreviews() {
  await invoke('stop_camera_preview').catch(() => {})
  await invoke('stop_mic_preview').catch(() => {})
}

/** Append an unlisten callback, chaining with any previous one. */
function chainUnlisten(
  ref: React.MutableRefObject<UnlistenFn | null>,
  unlisten: UnlistenFn
) {
  const prev = ref.current
  ref.current = () => {
    unlisten()
    prev?.()
  }
}

function PreJoinScreen({
  roomUrl,
  username,
  roomDisplayName,
  lang,
  isDark,
  onJoin,
  onCancel,
  livekitUrl,
  livekitToken,
}: Readonly<{
  roomUrl: string
  username: string | null
  roomDisplayName?: string | null
  lang: string
  isDark: boolean
  onJoin: (username: string | null) => void
  onCancel: () => void
  livekitUrl?: string | null
  livekitToken?: string | null
}>) {
  const t = useCallback(
    (key: string) => translations[lang]?.[key] ?? translations.en[key] ?? key,
    [lang]
  )

  const slugSource = roomUrl.split('?')[0]
  const slug = slugSource.includes('/')
    ? slugSource.split('/').pop()
    : slugSource

  // ---- State ---------------------------------------------------------------
  const [displayName, setDisplayName] = useState(username ?? '')
  const [isCameraOn, setIsCameraOn] = useState(false)
  const [isMicOn, setIsMicOn] = useState(true)
  const [audioMode, setAudioMode] = useState<'computer' | 'none'>('computer')
  const [previewFrame, setPreviewFrame] = useState<string | null>(null)
  // Unified device enumeration (lobby context — enumerates on mount)
  const devices = useDeviceEnumeration({
    onInputFallback: () => {
      invoke('stop_mic_preview')
        .catch(() => {})
        .then(() => invoke('start_mic_preview'))
        .catch(() => {})
    },
  })
  const {
    audioInputs: inputDevices,
    audioOutputs: outputDevices,
    videoInputs: videoDevices,
    selectedAudioInput: selectedInput,
    selectedAudioOutput: selectedOutput,
    selectedVideoInput: selectedCamera,
    setSelectedAudioInput: setSelectedInput,
    setSelectedAudioOutput: setSelectedOutput,
    setSelectedVideoInput: setSelectedCamera,
  } = devices
  const [micLevel, setMicLevel] = useState(0)
  const [showFilters, setShowFilters] = useState(false)
  const [backgroundMode, setBackgroundMode] = useState('off')

  // Close filter panel on Escape
  useEffect(() => {
    if (!showFilters) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setShowFilters(false)
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [showFilters])
  const [waitingState, setWaitingState] = useState<
    'idle' | 'waiting' | 'denied' | 'timeout'
  >('idle')

  // ---- Refs ----------------------------------------------------------------
  const micPollRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const unlistenVideoRef = useRef<UnlistenFn | null>(null)

  // ---- Effect: load settings and device lists on mount --------------------
  useEffect(() => {
    // Load saved settings
    invoke<Settings>('get_settings')
      .then((s) => {
        if (s.display_name) setDisplayName(s.display_name)
        setIsMicOn(s.mic_enabled_on_join ?? true)
        setIsCameraOn(s.camera_enabled_on_join ?? false)
        if (s.audio_mode === 'none') setAudioMode('none')
      })
      .catch(() => {})

    // Load device lists via unified hook
    devices.enumerate()

    // Subscribe to video frame events
    listen<{ track_sid: string; data: string; width: number; height: number }>(
      'video-frame',
      (event) => {
        if (event.payload.track_sid === 'local-camera') {
          setPreviewFrame(event.payload.data)
        }
      }
    ).then((unlisten) => {
      unlistenVideoRef.current = unlisten
    })

    return () => {
      unlistenVideoRef.current?.()
      if (micPollRef.current) clearInterval(micPollRef.current)
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
      // Stop previews on unmount
      invoke('stop_camera_preview').catch(() => {})
      invoke('stop_mic_preview').catch(() => {})
    }
  }, [])

  // ---- Effect: camera on/off ----------------------------------------------
  useEffect(() => {
    if (isCameraOn) {
      invoke('start_camera_preview').catch(() => {})
    } else {
      invoke('stop_camera_preview').catch(() => {})
      setPreviewFrame(null)
    }
  }, [isCameraOn])

  // ---- Effect: mic on/off + audioMode -------------------------------------
  useEffect(() => {
    if (isMicOn && audioMode === 'computer') {
      invoke('start_mic_preview').catch(() => {})
      micPollRef.current = setInterval(async () => {
        try {
          const level = await invoke<number>('get_mic_level')
          setMicLevel(level)
        } catch {
          setMicLevel(0)
        }
      }, 100)
    } else {
      invoke('stop_mic_preview').catch(() => {})
      if (micPollRef.current) {
        clearInterval(micPollRef.current)
        micPollRef.current = null
      }
      setMicLevel(0)
    }
    return () => {
      if (micPollRef.current) {
        clearInterval(micPollRef.current)
        micPollRef.current = null
      }
    }
  }, [isMicOn, audioMode])

  // Audio device fallback is now handled by useDeviceEnumeration hook.

  // ---- Handlers -----------------------------------------------------------
  const handleSelectCamera = async (uniqueId: string) => {
    setSelectedCamera(uniqueId)
    try {
      await invoke('set_camera_device', { uniqueId: uniqueId || null })
    } catch {
      /* ignore */
    }
    if (isCameraOn) {
      try {
        await invoke('stop_camera_preview')
      } catch {
        /* ignore */
      }
      try {
        await invoke('start_camera_preview')
      } catch {
        /* ignore */
      }
    }
  }

  const handleSelectInput = async (name: string) => {
    setSelectedInput(name)
    try {
      await invoke('select_audio_input', { deviceName: name })
    } catch {
      /* ignore */
    }
  }

  const handleSelectOutput = async (name: string) => {
    setSelectedOutput(name)
    try {
      await invoke('select_audio_output', { deviceName: name })
    } catch {
      /* ignore */
    }
  }

  const handleSetBackgroundMode = async (mode: string) => {
    setBackgroundMode(mode)
    try {
      if (mode.startsWith('image:')) {
        const id = Number.parseInt(mode.slice(6), 10)
        const path = await resolveResource(`backgrounds/${id}.jpg`)
        await invoke('load_background_image', { id, jpegPath: path })
      }
      await invoke('set_background_mode', { mode })
    } catch {
      /* ignore */
    }
  }

  const handleJoinNow = async () => {
    const finalName = displayName.trim() || null

    await savePreJoinPreferences(finalName, isMicOn, isCameraOn, audioMode)
    await stopPreviews()

    // When the room creator has LiveKit credentials from room creation,
    // connect directly using connect_with_token to bypass the lobby.
    // This avoids the "waiting for authorization" state for public rooms
    // where the creator would otherwise be stuck waiting for self-approval.
    if (livekitUrl && livekitToken) {
      try {
        await invoke('connect_with_token', { livekitUrl, token: livekitToken })
        onJoin(finalName)
      } catch (e) {
        console.error('connect_with_token failed:', e)
        setWaitingState('idle')
      }
      return
    }

    setWaitingState('waiting')

    // Start 60s timeout
    timeoutRef.current = setTimeout(() => {
      setWaitingState((prev) => (prev === 'waiting' ? 'timeout' : prev))
    }, 60_000)

    // Listen for lobby denied event
    listen<string>('lobby-denied', () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
      setWaitingState('denied')
    })
      .then((unlisten) => chainUnlisten(unlistenVideoRef, unlisten))
      .catch(() => {})

    // Connect to the room. For lobby-gated rooms, connect() returns Ok
    // immediately while the user is still waiting_for_host. We listen for the
    // connection-state-changed event and only call onJoin once the backend
    // transitions to "connected", avoiding a duplicate connect call from the
    // parent and preventing the blank-screen caused by setView("call") firing
    // before admission.
    try {
      await invoke('connect', { meetUrl: roomUrl, username: finalName })
    } catch {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
      setWaitingState('idle')
      return
    }

    listen<string>('connection-state-changed', (event) => {
      if (event.payload === 'connected') {
        if (timeoutRef.current) clearTimeout(timeoutRef.current)
        onJoin(finalName)
      }
    })
      .then((unlisten) => chainUnlisten(unlistenVideoRef, unlisten))
      .catch(() => {})
  }

  const handleBack = () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current)
    setWaitingState('idle')
    onCancel()
  }

  // ---- Waiting state overlay ----------------------------------------------
  if (waitingState !== 'idle') {
    return (
      <div
        className="prejoin-waiting-overlay"
        data-theme={isDark ? 'dark' : 'light'}
      >
        <div className="prejoin-waiting-content">
          {waitingState === 'waiting' && (
            <>
              <div className="prejoin-spinner" />
              <p className="prejoin-waiting-label">
                {t('prejoin.waitingForApproval')}
              </p>
              <button className="btn btn-secondary" onClick={handleBack}>
                {t('prejoin.cancel')}
              </button>
            </>
          )}
          {waitingState === 'denied' && (
            <>
              <p className="prejoin-waiting-error">
                {t('prejoin.accessDenied')}
              </p>
              <button className="btn btn-secondary" onClick={handleBack}>
                {t('prejoin.backToHome')}
              </button>
            </>
          )}
          {waitingState === 'timeout' && (
            <>
              <p className="prejoin-waiting-error">
                {t('prejoin.requestTimeout')}
              </p>
              <button className="btn btn-secondary" onClick={handleBack}>
                {t('prejoin.backToHome')}
              </button>
            </>
          )}
        </div>
      </div>
    )
  }

  // ---- Main layout --------------------------------------------------------
  const selectedCamName =
    videoDevices.find((d) => d.unique_id === selectedCamera)?.name ?? ''
  const selectedInputName =
    inputDevices.find((d) => d.name === selectedInput)?.name ?? selectedInput
  const selectedOutputName =
    outputDevices.find((d) => d.name === selectedOutput)?.name ?? selectedOutput

  return (
    <div className="prejoin-container" data-theme={isDark ? 'dark' : 'light'}>
      {/* Header */}
      <div className="prejoin-header">
        <span className="prejoin-app-name">Visio Mobile</span>
        {roomDisplayName ? (
          <>
            <span className="prejoin-slug">{roomDisplayName}</span>
            <span className="prejoin-slug-secondary">{slug}</span>
          </>
        ) : (
          <span className="prejoin-slug">{slug}</span>
        )}
      </div>

      {/* Display name */}
      <div className="prejoin-name-row">
        <input
          className="prejoin-name-input"
          type="text"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          placeholder={t('prejoin.displayName')}
          maxLength={100}
        />
      </div>

      {/* Two-column body */}
      <div className="prejoin-body">
        {/* Left: camera preview */}
        <div className="prejoin-camera-panel">
          <div className="prejoin-preview">
            {isCameraOn && previewFrame ? (
              <img
                className="prejoin-preview-img"
                src={`data:image/jpeg;base64,${previewFrame}`}
                alt=""
              />
            ) : (
              <div className="prejoin-preview-off">
                <RiVideoOffLine size={40} color="var(--text-secondary)" />
              </div>
            )}
          </div>

          {/* Camera device row */}
          <div className="prejoin-device-row">
            <div className="prejoin-device-selector">
              <RiVideoOnLine size={16} />
              <select
                className="prejoin-select"
                value={selectedCamera}
                onChange={(e) => handleSelectCamera(e.target.value)}
                title={t('prejoin.camera')}
              >
                {videoDevices.length === 0 && (
                  <option value="">{t('prejoin.camera')}</option>
                )}
                {videoDevices.map((d) => (
                  <option key={d.unique_id} value={d.unique_id}>
                    {d.name}
                  </option>
                ))}
                {videoDevices.length > 0 && selectedCamera === '' && (
                  <option value="">
                    {selectedCamName || t('prejoin.camera')}
                  </option>
                )}
              </select>
            </div>
            <button
              className={`prejoin-toggle${isCameraOn ? ' active' : ''}`}
              onClick={() => setIsCameraOn((v) => !v)}
              title={t('prejoin.camera')}
            >
              {isCameraOn ? (
                <RiVideoOnLine size={18} />
              ) : (
                <RiVideoOffLine size={18} />
              )}
            </button>
          </div>

          {/* Background filter row */}
          <button
            className={`prejoin-filter-btn${showFilters ? ' active' : ''}`}
            onClick={() => setShowFilters((v) => !v)}
          >
            <span>{t('prejoin.backgroundFilters')}</span>
            <RiArrowRightSLine size={16} />
          </button>
        </div>

        {/* Right: audio panel */}
        <div className="prejoin-audio-panel">
          {/* Computer audio option */}
          <label
            className={`prejoin-audio-option${audioMode === 'computer' ? ' selected' : ''}`}
          >
            <input
              type="radio"
              name="audioMode"
              value="computer"
              checked={audioMode === 'computer'}
              onChange={() => setAudioMode('computer')}
            />
            <span className="prejoin-audio-option-label">
              {t('prejoin.computerAudio')}
            </span>
          </label>

          {audioMode === 'computer' && (
            <div className="prejoin-audio-details">
              {/* Mic row */}
              <div className="prejoin-device-row">
                <div className="prejoin-device-selector">
                  <RiMicLine size={16} />
                  <select
                    className="prejoin-select"
                    value={selectedInput}
                    onChange={(e) => handleSelectInput(e.target.value)}
                    title={t('prejoin.microphone')}
                  >
                    {inputDevices.length === 0 && (
                      <option value="">{t('prejoin.microphone')}</option>
                    )}
                    {inputDevices.map((d) => (
                      <option key={d.name} value={d.name}>
                        {d.name}
                      </option>
                    ))}
                    {inputDevices.length > 0 && selectedInput === '' && (
                      <option value="">
                        {selectedInputName || t('prejoin.microphone')}
                      </option>
                    )}
                  </select>
                </div>
                <button
                  className={`prejoin-toggle${isMicOn ? ' active' : ''}`}
                  onClick={() => setIsMicOn((v) => !v)}
                  title={t('prejoin.microphone')}
                >
                  {isMicOn ? (
                    <RiMicLine size={18} />
                  ) : (
                    <RiMicOffLine size={18} />
                  )}
                </button>
              </div>

              {/* VU meter */}
              <div className="prejoin-vu-track" data-testid="prejoin-vu-track">
                <div
                  className="prejoin-vu-bar"
                  data-testid="prejoin-vu-bar"
                  style={{
                    width: `${Math.round(Math.min(micLevel * 25, 1) * 100)}%`,
                  }}
                />
              </div>

              {/* Speaker row */}
              <div className="prejoin-device-row prejoin-speaker-row">
                <div className="prejoin-device-selector">
                  <RiVolumeMuteLine size={16} />
                  <select
                    className="prejoin-select"
                    value={selectedOutput}
                    onChange={(e) => handleSelectOutput(e.target.value)}
                    title="Speaker"
                  >
                    {outputDevices.length === 0 && (
                      <option value="">Speaker</option>
                    )}
                    {outputDevices.map((d) => (
                      <option key={d.name} value={d.name}>
                        {d.name}
                      </option>
                    ))}
                    {outputDevices.length > 0 && selectedOutput === '' && (
                      <option value="">
                        {selectedOutputName || t('device.speaker')}
                      </option>
                    )}
                  </select>
                </div>
              </div>
              <button
                className="btn btn-secondary prejoin-test-btn"
                onClick={() => invoke('play_speaker_test').catch(() => {})}
              >
                {t('prejoin.testSpeaker')}
              </button>
            </div>
          )}

          {/* No audio option */}
          <label
            className={`prejoin-audio-option${audioMode === 'none' ? ' selected' : ''}`}
          >
            <input
              type="radio"
              name="audioMode"
              value="none"
              checked={audioMode === 'none'}
              onChange={() => setAudioMode('none')}
            />
            <span className="prejoin-audio-option-label">
              {t('prejoin.noAudio')}
            </span>
          </label>
        </div>
      </div>

      {/* Actions */}
      <div className="prejoin-actions">
        <button className="btn btn-secondary" onClick={onCancel}>
          {t('prejoin.cancel')}
        </button>
        <button className="btn btn-primary" onClick={handleJoinNow}>
          {t('prejoin.joinNow')}
        </button>
      </div>

      {/* Background filter side panel */}
      {showFilters && (
        <div className="prejoin-filter-panel">
          <div className="prejoin-filter-panel-header">
            <span>{t('prejoin.backgroundFilters')}</span>
            <button onClick={() => setShowFilters(false)}>
              <RiCloseLine size={20} />
            </button>
          </div>
          <div className="prejoin-filter-grid">
            {/* Off */}
            <button
              className={`prejoin-filter-thumb${backgroundMode === 'off' ? ' active' : ''}`}
              onClick={() => handleSetBackgroundMode('off')}
            >
              <div className="prejoin-filter-thumb-off" />
              <span>{t('prejoin.bgOff')}</span>
            </button>
            {/* Blur */}
            <button
              className={`prejoin-filter-thumb${backgroundMode === 'blur' ? ' active' : ''}`}
              onClick={() => handleSetBackgroundMode('blur')}
            >
              <div className="prejoin-filter-thumb-blur" />
              <span>{t('prejoin.bgBlur')}</span>
            </button>
            {/* Blur light */}
            <button
              className={`prejoin-filter-thumb${backgroundMode === 'blur-light' ? ' active' : ''}`}
              onClick={() => handleSetBackgroundMode('blur-light')}
            >
              <div className="prejoin-filter-thumb-blur-light" />
              <span>{t('prejoin.bgBlurLight')}</span>
            </button>
            {/* Background images 1-8 */}
            {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => (
              <button
                key={n}
                className={`prejoin-filter-thumb${backgroundMode === 'image:' + n ? ' active' : ''}`}
                onClick={() => handleSetBackgroundMode('image:' + n)}
              >
                <img
                  src={`/backgrounds/thumbnails/${n}.jpg`}
                  alt={`Background ${n}`}
                  draggable={false}
                  style={{
                    width: '100%',
                    height: '100%',
                    objectFit: 'cover',
                    borderRadius: 6,
                  }}
                />
                <span>{n}</span>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// App (root)
// ---------------------------------------------------------------------------

export default function App() {
  const [view, setView] = useState<View>('home')
  const [lobbyRoomUrl, setLobbyRoomUrl] = useState('')
  const [lobbyUsername, setLobbyUsername] = useState<string | null>(null)
  const [lobbyLivekitUrl, setLobbyLivekitUrl] = useState<string | null>(null)
  const [lobbyLivekitToken, setLobbyLivekitToken] = useState<string | null>(
    null
  )
  const [currentRoomDisplayName, setCurrentRoomDisplayName] = useState<
    string | null
  >(null)
  const [connectionState, setConnectionState] = useState('disconnected')
  const [participants, setParticipants] = useState<Participant[]>([])
  const [localParticipant, setLocalParticipant] = useState<Participant | null>(
    null
  )
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [micEnabled, setMicEnabled] = useState(false)
  const [camEnabled, setCamEnabled] = useState(false)
  const [videoFrames, setVideoFrames] = useState<Map<string, string>>(
    () => new Map()
  )

  // New state for UX overhaul
  const [isHandRaised, setIsHandRaised] = useState(false)
  const [unreadCount, setUnreadCount] = useState(0)
  const [handRaisedMap, setHandRaisedMap] = useState<Record<string, number>>({})
  const [activeSpeakers, setActiveSpeakers] = useState<string[]>([])
  const [showChat, setShowChat] = useState(false)
  const [showParticipants, setShowParticipants] = useState(false)
  const [showInfo, setShowInfo] = useState(false)
  const [showTranscription, setShowTranscription] = useState(false)
  const [showMicPicker, setShowMicPicker] = useState(false)
  const [showCamPicker, setShowCamPicker] = useState(false)
  // Lobby / waiting room
  const [waitingParticipants, setWaitingParticipants] = useState<
    Array<{ id: string; username: string }>
  >([])
  // lobbyNotification removed — banner now driven by waitingParticipants directly

  // Deep link
  const [deepLinkUrl, setDeepLinkUrl] = useState<string | null>(null)
  const [deepLinkError, setDeepLinkError] = useState<string | null>(null)
  // Meeting URL (set on join, used in info panel)
  const [currentMeetUrl, setCurrentMeetUrl] = useState('')
  const [currentRoomId, setCurrentRoomId] = useState<string | null>(null)
  const [currentAccessLevel, setCurrentAccessLevel] = useState<string>('')
  // Display name (shared between Home and Settings)
  const [displayName, setDisplayName] = useState('')
  // i18n
  const [lang, setLang] = useState(detectSystemLang)
  // Theme
  const [theme, setTheme] = useState('light')
  // OIDC auth
  const [isAuthenticated, setIsAuthenticated] = useState(false)
  const [displayNameFromOidc, setDisplayNameFromOidc] = useState('')
  const [emailFromOidc, setEmailFromOidc] = useState('')
  const [authenticatedMeetInstance, setAuthenticatedMeetInstance] = useState('')
  const [meetInstances, setMeetInstances] = useState<string[]>([])
  const [pendingOidcInstance, setPendingOidcInstance] = useState<string | null>(
    null
  )
  const pendingOidcRef = useRef<string | null>(null)
  const [bandwidthMode, setBandwidthMode] = useState<string>('full')
  const settingsRef = useRef<Settings | null>(null)

  const t = useCallback(
    (key: string) => translations[lang]?.[key] ?? translations.en[key] ?? key,
    [lang]
  )

  // Load settings on mount
  useEffect(() => {
    invoke<Settings>('get_settings')
      .then((s) => {
        settingsRef.current = s
        if (s.display_name) setDisplayName(s.display_name)
        if (s.language) setLang(s.language)
        if (s.theme) setTheme(s.theme)
      })
      .catch(() => {})
    // Load session state
    invoke<{
      state: string
      display_name?: string
      email?: string
      meet_instance?: string
    }>('get_session_state')
      .then((result) => {
        if (result.state === 'authenticated') {
          setIsAuthenticated(true)
          setDisplayNameFromOidc(result.display_name || '')
          setEmailFromOidc(result.email || '')
          if (result.meet_instance)
            setAuthenticatedMeetInstance(result.meet_instance)
        }
      })
      .catch(() => {})
    // Load meet instances for OIDC
    invoke<string[]>('get_meet_instances')
      .then(setMeetInstances)
      .catch(() => {})

    // Load ONNX segmentation model for background blur
    resolveResource('models/selfie_segmentation.onnx')
      .then((path) => invoke('load_blur_model', { modelPath: path }))
      .catch(() => {})
  }, [])

  // Deep link listener
  useEffect(() => {
    const unlisten = onOpenUrl((urls: string[]) => {
      if (urls.length === 0) return
      const url = urls[0]
      try {
        const parsed = new URL(url)
        if (parsed.protocol !== 'visio:') return
        const host = parsed.hostname

        // Handle OIDC auth callback: visio://auth-callback?code={uuid}
        if (host === 'auth-callback') {
          const code = parsed.searchParams.get('code')
          const meetInstance = pendingOidcRef.current
          if (code && meetInstance) {
            pendingOidcRef.current = null
            setPendingOidcInstance(null)
            invoke<{
              display_name?: string
              email?: string
              meet_instance?: string
            }>('exchange_oidc_code', { meetInstance, code })
              .then((result) => {
                setIsAuthenticated(true)
                setAuthenticatedMeetInstance(meetInstance)
                setDisplayNameFromOidc(result.display_name || '')
                setEmailFromOidc(result.email || '')
                if (result.display_name && !displayName.trim()) {
                  setDisplayName(result.display_name)
                }
                if (!meetInstances.includes(meetInstance)) {
                  const next = [...meetInstances, meetInstance]
                  setMeetInstances(next)
                  invoke('set_meet_instances', { instances: next })
                }
              })
              .catch((e) => {
                console.error('OIDC code exchange failed:', e)
              })
          }
          return
        }

        // Handle room deep links: visio://{host}/{slug}[?visio=...]
        const pathSegment = parsed.pathname.replace(/^\//, '')
        if (!host || !pathSegment) return

        const deepLinkDisplayName = parsed.searchParams.get('visio')
        invoke<string[]>('get_meet_instances').then(async (instances) => {
          if (!instances.includes(host)) {
            setDeepLinkError(
              t('deepLink.unknownInstance').replace('{host}', host)
            )
            return
          }

          // If path is a valid slug, use directly
          if (SLUG_REGEX.test(pathSegment)) {
            setView('home')
            let roomUrl = `https://${host}/${pathSegment}`
            if (deepLinkDisplayName) {
              roomUrl += `?visio=${encodeURIComponent(deepLinkDisplayName)}`
            }
            setDeepLinkUrl(roomUrl)
            setDeepLinkError(null)
            return
          }

          // Otherwise try alias resolution
          try {
            const resolved = await invoke<string | null>('resolve_visio_alias', { name: pathSegment })
            if (resolved) {
              setView('home')
              let roomUrl = resolved
              if (deepLinkDisplayName) {
                roomUrl += `?visio=${encodeURIComponent(deepLinkDisplayName)}`
              }
              setDeepLinkUrl(roomUrl)
              setDeepLinkError(null)
            } else {
              setDeepLinkError(
                t('error.unknownAlias').replace('{name}', pathSegment)
              )
            }
          } catch {
            setDeepLinkError(
              t('error.unknownAlias').replace('{name}', pathSegment)
            )
          }
        })
      } catch {
        /* ignore malformed URLs */
      }
    })
    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  // Auto-connect listener (CLI args: --livekit-url <url> --token <token>)
  useEffect(() => {
    const unlisten = listen<{ livekit_url: string; token: string }>(
      'auto-connect',
      async (event) => {
        const { livekit_url, token } = event.payload
        try {
          await invoke('connect_with_token', { livekitUrl: livekit_url, token })
          setCurrentMeetUrl(livekit_url)
          setView('call')

          // Auto-chat messages for E2E test (turn-based)
          const messages = [
            { delay: 3000, text: 'Desktop joined the room!' },
            { delay: 25000, text: 'Desktop: my turn to speak!' },
            { delay: 35000, text: 'Desktop: screen sharing active' },
            { delay: 50000, text: "Desktop: muted — Android's turn" },
            { delay: 100000, text: 'Desktop: everyone speaking together!' },
          ]
          for (const msg of messages) {
            setTimeout(async () => {
              try {
                await invoke('send_chat', { text: msg.text })
              } catch {}
            }, msg.delay)
          }

          // Turn-based speaking: Desktop speaks at 25-50s, muted otherwise (except warmup 0-5s and final 100-120s)
          // 5s: mute mic+cam (bot's turn)
          setTimeout(async () => {
            try {
              await invoke('toggle_mic', { enabled: false })
              await invoke('toggle_camera', { enabled: false })
              console.log("[TURN] Desktop muted (bot's turn)")
            } catch {}
          }, 5000)
          // 25s: unmute — Desktop's turn to speak
          setTimeout(async () => {
            try {
              await invoke('toggle_mic', { enabled: true })
              await invoke('toggle_camera', { enabled: true })
              console.log('[TURN] Desktop speaking')
            } catch {}
          }, 25000)
          // 50s: mute — Android's turn
          setTimeout(async () => {
            try {
              await invoke('toggle_mic', { enabled: false })
              await invoke('toggle_camera', { enabled: false })
              console.log("[TURN] Desktop muted (Android's turn)")
            } catch {}
          }, 50000)
          // 100s: unmute — everyone speaks
          setTimeout(async () => {
            try {
              await invoke('toggle_mic', { enabled: true })
              await invoke('toggle_camera', { enabled: true })
              console.log('[TURN] Desktop unmuted (all speak)')
            } catch {}
          }, 100000)

          // Auto screen share during Desktop's turn (30-48s)
          setTimeout(async () => {
            try {
              const sources = await invoke<
                Array<{ id: string; name: string; source_type: string }>
              >('list_screen_sources')
              const monitor =
                sources.find((s) => s.source_type === 'Monitor') || sources[0]
              if (monitor) {
                console.log('[TURN] Desktop screen share started')
                await invoke('start_screen_share', { sourceId: monitor.id })
                setTimeout(async () => {
                  try {
                    await invoke('stop_screen_share')
                    console.log('[TURN] Desktop screen share stopped')
                  } catch (err) {
                    console.error('Screen share stop failed:', err)
                  }
                }, 18000)
              }
            } catch (err) {
              console.error('Screen share failed:', err)
            }
          }, 30000)
        } catch (err) {
          console.error('Auto-connect failed:', err)
        }
      }
    )
    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  // Apply theme to document
  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  // ---- Unified device enumeration (in-call context) -----------------------
  // Lazy: does NOT enumerate until a picker is opened (avoids macOS mic
  // permission issue #161). Fallback + audio-devices-changed handled by hook.
  const inCallDevices = useDeviceEnumeration()
  const {
    audioInputs,
    audioOutputs,
    videoInputs,
    selectedAudioInput,
    selectedAudioOutput,
    selectedVideoInput,
    setSelectedAudioInput,
    setSelectedAudioOutput,
    setSelectedVideoInput,
    devicesEnumerated,
    enumerate: enumerateDevices,
  } = inCallDevices

  const viewRef = useRef(view)
  viewRef.current = view

  // Trigger enumeration lazily when a device picker is first opened.
  useEffect(() => {
    if ((!showMicPicker && !showCamPicker) || devicesEnumerated) return
    enumerateDevices()
  }, [showMicPicker, showCamPicker, devicesEnumerated, enumerateDevices])

  // ---- Click outside to close device pickers ------------------------------
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (!(e.target as Element).closest('.device-picker, .control-chevron')) {
        setShowMicPicker(false)
        setShowCamPicker(false)
      }
    }
    document.addEventListener('click', handleClick)
    return () => document.removeEventListener('click', handleClick)
  }, [])

  // ---- Polling ------------------------------------------------------------
  const poll = useCallback(async () => {
    try {
      const state: string = await invoke('get_connection_state')
      setConnectionState(state)

      if (
        state === 'disconnected' &&
        viewRef.current !== 'home' &&
        viewRef.current !== 'settings'
      ) {
        setView('home')
        setMicEnabled(false)
        setCamEnabled(false)
        setMessages([])
        setVideoFrames(new Map())
        setShowChat(false)
        setShowParticipants(false)
        setShowInfo(false)
        setShowTranscription(false)
        setIsHandRaised(false)
        setUnreadCount(0)
        setHandRaisedMap({})
        setActiveSpeakers([])
        setLocalParticipant(null)
        return
      }

      if (state === 'connected' || state === 'reconnecting') {
        const ps: Participant[] = await invoke('get_participants')
        setParticipants(ps)

        const lp: Participant | null = await invoke('get_local_participant')
        setLocalParticipant(lp)

        const ms: ChatMessage[] = await invoke('get_messages')
        setMessages(ms)
      }
    } catch (e) {
      console.error('poll error:', e)
    }
  }, [])

  useEffect(() => {
    if (view === 'home' || view === 'lobby') return

    poll()
    const id = setInterval(poll, 1000)
    return () => clearInterval(id)
  }, [view, poll])

  // ---- Video frame events -------------------------------------------------
  useEffect(() => {
    if (view === 'home' || view === 'lobby') return

    let unlistenFrame: UnlistenFn | null = null
    let unlistenTrackUnsub: UnlistenFn | null = null

    // Backpressure: batch frame updates and throttle to ~20 fps (50ms)
    // to avoid overwhelming React with state updates when many participants
    // are streaming video simultaneously.  The previous approach used
    // requestAnimationFrame which still fires at 60fps — too fast when
    // 12+ participants each push frames.
    let pendingFrames: Map<string, string> = new Map()
    let pendingRemovals: Set<string> = new Set()
    let flushTimer: ReturnType<typeof setTimeout> | null = null
    const FLUSH_INTERVAL_MS = 50 // cap at ~20 fps

    const flushFrames = () => {
      flushTimer = null
      if (pendingFrames.size === 0 && pendingRemovals.size === 0) return
      const batch = pendingFrames
      const removals = pendingRemovals
      pendingFrames = new Map()
      pendingRemovals = new Set()
      setVideoFrames((prev) => {
        const next = new Map(prev)
        // Remove unsubscribed tracks first
        for (const sid of removals) {
          next.delete(sid)
        }
        // Apply new frames
        for (const [sid, d] of batch) {
          next.set(sid, d)
        }
        return next
      })
    }

    const scheduleFlush = () => {
      if (flushTimer === null) {
        flushTimer = setTimeout(flushFrames, FLUSH_INTERVAL_MS)
      }
    }

    listen<VideoFrame>('video-frame', (event) => {
      const { track_sid, data } = event.payload
      pendingFrames.set(track_sid, data)
      scheduleFlush()
    }).then((fn) => {
      unlistenFrame = fn
    })

    // Clean up video frames when tracks are unsubscribed — prevents the
    // videoFrames Map from growing unboundedly and leaking memory.
    listen<string>('track-unsubscribed', (event) => {
      const trackSid = event.payload
      pendingFrames.delete(trackSid)
      pendingRemovals.add(trackSid)
      scheduleFlush()
    }).then((fn) => {
      unlistenTrackUnsub = fn
    })

    return () => {
      unlistenFrame?.()
      unlistenTrackUnsub?.()
      if (flushTimer !== null) clearTimeout(flushTimer)
    }
  }, [view])

  // ---- Hand raise & unread events (Task 2.8) ------------------------------
  useEffect(() => {
    if (view === 'home' || view === 'lobby') return

    let unlistenHand: UnlistenFn | null = null
    let unlistenUnread: UnlistenFn | null = null
    let unlistenSpeakers: UnlistenFn | null = null
    let unlistenBandwidth: UnlistenFn | null = null

    listen<{ participantSid: string; raised: boolean; position: number }>(
      'hand-raised-changed',
      (event) => {
        const { participantSid, raised, position } = event.payload
        setHandRaisedMap((prev) => ({
          ...prev,
          [participantSid]: raised ? position : 0,
        }))
        // If our own hand was auto-lowered
        // We don't have localSid here, but we track via isHandRaised
        if (!raised) {
          // Check via invoke if our hand is still raised
          invoke<boolean>('is_hand_raised').then((val) => {
            setIsHandRaised(val)
          })
        }
      }
    ).then((fn) => {
      unlistenHand = fn
    })

    listen<number>('unread-count-changed', (event) => {
      setUnreadCount(event.payload)
    }).then((fn) => {
      unlistenUnread = fn
    })

    listen<string[]>('active-speakers-changed', (event) => {
      setActiveSpeakers(event.payload)
    }).then((fn) => {
      unlistenSpeakers = fn
    })

    listen<string>('bandwidth-mode-changed', (event) => {
      setBandwidthMode(event.payload)
    }).then((fn) => {
      unlistenBandwidth = fn
    })

    return () => {
      unlistenHand?.()
      unlistenUnread?.()
      unlistenSpeakers?.()
      unlistenBandwidth?.()
    }
  }, [view])

  // ---- Lobby events -------------------------------------------------------
  useEffect(() => {
    let unlistenDenied: UnlistenFn | null = null
    let unlistenTimeout: UnlistenFn | null = null
    let unlistenJoined: UnlistenFn | null = null
    let unlistenLeft: UnlistenFn | null = null

    listen('lobby-denied', () => {
      setConnectionState('disconnected')
      setView('home')
      alert(t('lobby.denied'))
    }).then((fn) => {
      unlistenDenied = fn
    })

    listen('lobby-timeout', () => {
      setConnectionState('disconnected')
      setView('home')
      alert(t('lobby.timeout'))
    }).then((fn) => {
      unlistenTimeout = fn
    })

    listen<{ id: string; username: string }>(
      'lobby-participant-joined',
      (event) => {
        const p = event.payload
        setWaitingParticipants((prev) => {
          if (prev.some((x) => x.id === p.id)) return prev
          return [...prev, p]
        })
      }
    ).then((fn) => {
      unlistenJoined = fn
    })

    listen<{ id: string }>('lobby-participant-left', (event) => {
      const { id } = event.payload
      setWaitingParticipants((prev) => prev.filter((x) => x.id !== id))
    }).then((fn) => {
      unlistenLeft = fn
    })

    return () => {
      unlistenDenied?.()
      unlistenTimeout?.()
      unlistenJoined?.()
      unlistenLeft?.()
    }
  }, [t])

  // ---- Handlers -----------------------------------------------------------
  const handleJoin = async (
    meetUrl: string,
    username?: string | null,
    roomId?: string,
    accessLevel?: string,
    livekitUrl?: string,
    livekitToken?: string
  ) => {
    // Extract room display name from query param before storing URL
    let displayNameFromUrl: string | null = null
    try {
      const parsed = new URL(
        meetUrl.startsWith('http') ? meetUrl : `https://${meetUrl}`
      )
      const raw = parsed.searchParams.get('visio')
      if (raw) displayNameFromUrl = decodeURIComponent(raw)
    } catch {
      /* ignore */
    }
    setCurrentRoomDisplayName(displayNameFromUrl)
    setCurrentMeetUrl(meetUrl)
    if (roomId) setCurrentRoomId(roomId)
    if (accessLevel) setCurrentAccessLevel(accessLevel)
    setLobbyRoomUrl(meetUrl)
    setLobbyUsername(username ?? null)
    setLobbyLivekitUrl(livekitUrl && livekitUrl.length > 0 ? livekitUrl : null)
    setLobbyLivekitToken(
      livekitToken && livekitToken.length > 0 ? livekitToken : null
    )
    setView('lobby')
  }

  const handleToggleMic = async () => {
    const next = !micEnabled
    setMicEnabled(next)
    try {
      await invoke('toggle_mic', { enabled: next })
    } catch (e) {
      console.error('mic toggle error:', e)
      setMicEnabled(!next)
    }
  }

  // Push-to-talk: hold Space to temporarily unmute
  const pushToTalkRef = useRef(false)
  useEffect(() => {
    if (view !== 'call') return
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.code !== 'Space' || e.repeat) return
      // Don't activate if typing in an input
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      e.preventDefault()
      if (!micEnabled && !pushToTalkRef.current) {
        pushToTalkRef.current = true
        setMicEnabled(true)
        try {
          await invoke('toggle_mic', { enabled: true })
        } catch {}
      }
    }
    const handleKeyUp = async (e: KeyboardEvent) => {
      if (e.code !== 'Space') return
      if (pushToTalkRef.current) {
        pushToTalkRef.current = false
        setMicEnabled(false)
        try {
          await invoke('toggle_mic', { enabled: false })
        } catch {}
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    window.addEventListener('keyup', handleKeyUp)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
      window.removeEventListener('keyup', handleKeyUp)
    }
  }, [view, micEnabled])

  const handleToggleCam = async () => {
    const next = !camEnabled
    setCamEnabled(next)
    try {
      await invoke('toggle_camera', { enabled: next })
    } catch (e) {
      console.error('camera toggle error:', e)
      setCamEnabled(!next)
    }
  }

  const handleHangUp = async () => {
    try {
      await invoke('disconnect')
    } catch (e) {
      console.error('disconnect error:', e)
    }
    setView('home')
    setMicEnabled(false)
    setCamEnabled(false)
    setMessages([])
    setVideoFrames(new Map())
    setShowChat(false)
    setShowParticipants(false)
    setShowInfo(false)
    setShowTranscription(false)
    setConnectionState('disconnected')
    setIsHandRaised(false)
    setUnreadCount(0)
    setHandRaisedMap({})
    setActiveSpeakers([])
    setLocalParticipant(null)
    setCurrentMeetUrl('')
    setBandwidthMode('full')
    setBandwidthMode('full')
  }

  const handleToggleHandRaise = async () => {
    try {
      if (isHandRaised) {
        await invoke('lower_hand')
      } else {
        await invoke('raise_hand')
      }
      setIsHandRaised(!isHandRaised)
    } catch (e) {
      console.error('hand raise error:', e)
    }
  }

  const handleToggleChat = async () => {
    const newState = !showChat
    setShowChat(newState)
    try {
      await invoke('set_chat_open', { open: newState })
    } catch (e) {
      console.error('set_chat_open error:', e)
    }
    if (newState) setUnreadCount(0)
  }

  const handleSendChat = async (text: string) => {
    try {
      await invoke('send_chat', { text })
    } catch (e) {
      console.error('send error:', e)
    }
  }

  // ---- Device selection handlers ------------------------------------------
  const handleSelectAudioInput = async (name: string) => {
    setSelectedAudioInput(name)
    try {
      await invoke('select_audio_input', { deviceName: name })
    } catch (e) {
      console.error('Failed to select audio input:', e)
    }
  }

  const handleSelectAudioOutput = async (name: string) => {
    setSelectedAudioOutput(name)
    try {
      await invoke('select_audio_output', { deviceName: name })
    } catch (e) {
      console.error('Failed to select audio output:', e)
    }
  }

  const handleSelectVideoInput = async (uniqueId: string) => {
    setSelectedVideoInput(uniqueId)
    try {
      await invoke('select_video_input', { uniqueId })
    } catch (e) {
      console.error('Failed to select video input:', e)
    }
  }

  // ---- Render -------------------------------------------------------------
  return (
    <I18nContext.Provider value={t}>
      {(view === 'call' || connectionState === 'waiting_for_host') && (
        <header>
          <h1>{currentRoomDisplayName || t('app.title')}</h1>
          <StatusBadge state={connectionState} />
        </header>
      )}
      {view === 'call' && bandwidthMode !== 'full' && (
        <div className="bandwidth-indicator">
          {bandwidthMode === 'reduced_video'
            ? t('bandwidth.reducedVideo')
            : t('bandwidth.audioOnly')}
        </div>
      )}
      <main>
        {view === 'home' && (
          <>
            <HomeView
              onJoin={handleJoin}
              onOpenSettings={() => setView('settings')}
              displayName={displayName}
              onDisplayNameChange={setDisplayName}
              deepLinkUrl={deepLinkUrl}
              onDeepLinkConsumed={() => setDeepLinkUrl(null)}
              isAuthenticated={isAuthenticated}
              authenticatedMeetInstance={authenticatedMeetInstance}
              displayNameFromOidc={displayNameFromOidc}
              emailFromOidc={emailFromOidc}
              onLaunchOidc={async (meetInstance: string) => {
                try {
                  setPendingOidcInstance(meetInstance)
                  pendingOidcRef.current = meetInstance
                  await invoke('launch_oidc_browser', { meetInstance })
                } catch (e) {
                  console.error('Failed to open browser for OIDC:', e)
                  setPendingOidcInstance(null)
                  pendingOidcRef.current = null
                }
              }}
              meetInstances={meetInstances}
              onLogout={() => {
                if (authenticatedMeetInstance) {
                  invoke('logout_session', {
                    meetUrl: `https://${authenticatedMeetInstance}`,
                  }).then(() => {
                    setIsAuthenticated(false)
                    setAuthenticatedMeetInstance('')
                    setDisplayNameFromOidc('')
                    setEmailFromOidc('')
                  })
                }
              }}
            />
            {deepLinkError && (
              <div className="deep-link-error">
                <span>{deepLinkError}</span>
                <button onClick={() => setDeepLinkError(null)}>
                  <RiCloseLine size={16} />
                </button>
              </div>
            )}
          </>
        )}
        {view === 'lobby' && (
          <PreJoinScreen
            roomUrl={lobbyRoomUrl}
            username={lobbyUsername}
            roomDisplayName={currentRoomDisplayName}
            lang={lang}
            isDark={theme === 'dark'}
            onJoin={() => {
              setView('call')
            }}
            onCancel={() => setView('home')}
            livekitUrl={lobbyLivekitUrl}
            livekitToken={lobbyLivekitToken}
          />
        )}
        {view === 'call' && connectionState !== 'waiting_for_host' && (
          <CallView
            participants={participants}
            localParticipant={localParticipant}
            micEnabled={micEnabled}
            camEnabled={camEnabled}
            videoFrames={videoFrames}
            messages={messages}
            handRaisedMap={handRaisedMap}
            activeSpeakers={activeSpeakers}
            isHandRaised={isHandRaised}
            unreadCount={unreadCount}
            showChat={showChat}
            onToggleMic={handleToggleMic}
            onToggleCam={handleToggleCam}
            onHangUp={handleHangUp}
            onToggleHandRaise={handleToggleHandRaise}
            onToggleChat={handleToggleChat}
            onSendChat={handleSendChat}
            onToggleParticipants={() => setShowParticipants(!showParticipants)}
            showParticipants={showParticipants}
            onToggleInfo={() => {
              setShowInfo(!showInfo)
              if (showInfo) setShowTranscription(false)
            }}
            showInfo={showInfo}
            meetUrl={currentMeetUrl}
            onToggleTranscription={() =>
              setShowTranscription(!showTranscription)
            }
            showTranscription={showTranscription}
            onShowMicPicker={() => {
              setShowMicPicker(!showMicPicker)
              setShowCamPicker(false)
            }}
            onShowCamPicker={() => {
              setShowCamPicker(!showCamPicker)
              setShowMicPicker(false)
            }}
            showMicPicker={showMicPicker}
            showCamPicker={showCamPicker}
            audioInputs={audioInputs}
            audioOutputs={audioOutputs}
            videoInputs={videoInputs}
            selectedAudioInput={selectedAudioInput}
            selectedAudioOutput={selectedAudioOutput}
            selectedVideoInput={selectedVideoInput}
            onSelectAudioInput={handleSelectAudioInput}
            onSelectAudioOutput={handleSelectAudioOutput}
            onSelectVideoInput={handleSelectVideoInput}
            waitingParticipants={waitingParticipants}
            setWaitingParticipants={setWaitingParticipants}
            roomId={currentRoomId || undefined}
            accessLevel={currentAccessLevel || undefined}
            roomDisplayName={currentRoomDisplayName}
            bandwidthMode={bandwidthMode}
          />
        )}
        {connectionState === 'waiting_for_host' && (
          <WaitingScreen
            t={t}
            onCancel={async () => {
              try {
                await invoke('cancel_lobby')
              } catch (_) {
                /* ignore */
              }
              try {
                await invoke('disconnect')
              } catch (_) {
                /* ignore */
              }
              setConnectionState('disconnected')
              setView('home')
            }}
          />
        )}
        {view === 'settings' && (
          <SettingsView
            onClose={() => {
              setView('home')
              invoke<string[]>('get_meet_instances')
                .then(setMeetInstances)
                .catch(() => {})
            }}
            onLanguageChange={(l) => setLang(l)}
            onThemeChange={(t) => setTheme(t)}
            onDisplayNameChange={setDisplayName}
            initialDisplayName={displayName}
          />
        )}
      </main>
    </I18nContext.Provider>
  )
}
