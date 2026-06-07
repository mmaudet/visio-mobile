import { useState, useEffect, useRef, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { resolveResource } from '@tauri-apps/api/path'
import { onOpenUrl } from '@tauri-apps/plugin-deep-link'
import {
  RiCheckLine,
  RiCloseLine,
  RiFileCopyLine,
  RiGlobalLine,
  RiSmartphoneLine,
} from '@remixicon/react'
import { useDeviceEnumeration } from './useDeviceEnumeration'
import { DeskWindow } from './components/layout/DeskWindow'
import { DeskSidebar, type NavKey } from './components/layout/DeskSidebar'
import { Button as VButton } from './components/ui/Button'
import { Icon as VIcon } from './components/Icon'
import { HomeScreen } from './screens/HomeScreen'
import { SettingsScreen } from './screens/SettingsScreen'
import { LobbyScreen } from './screens/LobbyScreen'
import { CallScreen } from './screens/CallScreen'
import type { ThemeChoice } from './types'

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

interface ChatMessage {
  id: string
  sender_sid: string
  sender_name: string | null
  text: string
  timestamp_ms: number
  encrypted: boolean
  decryption_failed: boolean
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

function detectSystemLang(): string {
  const navLang = navigator.language?.split('-')[0]
  return SUPPORTED_LANGS.includes(navLang) ? navLang : 'en'
}

function isDarkTheme(theme: string): boolean {
  if (theme === 'dark') return true
  if (theme === 'light') return false
  // 'system' or unknown: trust the media query
  return (
    typeof window !== 'undefined' &&
    window.matchMedia?.('(prefers-color-scheme: dark)').matches
  )
}

function profileDisplayName(local: string, fromOidc: string): string {
  return (fromOidc || local || '').trim() || 'Visio'
}

function profileSubtitle(
  isAuth: boolean,
  meetInstance: string,
  email: string
): string {
  if (!isAuth) return 'Anonyme'
  if (meetInstance && /\bgouv\.fr\b/.test(meetInstance)) return 'ProConnect'
  if (email) return email
  if (meetInstance) return meetInstance
  return ''
}

function firstName(full: string): string {
  const trimmed = full.trim()
  if (!trimmed || trimmed === 'Visio') return ''
  return trimmed.split(/\s+/)[0]
}

function isMeetingImminent(m: Meeting): boolean {
  const nowSec = Date.now() / 1000
  const minutesUntil = (m.start_time - nowSec) / 60
  return minutesUntil >= 0 && minutesUntil < 15
}

function isMeetingOngoing(m: Meeting): boolean {
  const now = Date.now() / 1000
  return m.start_time <= now && now <= m.end_time
}

// -- Create Room Dialog -----------------------------------------------------

function CreateRoomDialog({
  meetInstance,
  onCreated,
  onCancel,
  t,
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
  t: (key: string) => string
}>) {
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
        const conflict = await invoke<string | null>(
          'check_visio_alias_conflict',
          {
            name: trimmedName,
            url: baseUrl,
          }
        )
        if (conflict) {
          setAliasConflictName(trimmedName)
          setAliasConflictUrl(baseUrl)
        } else {
          await invoke('add_visio_alias', {
            name: trimmedName,
            url: baseUrl,
          }).catch(() => {})
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
              {roomDisplayName.trim() &&
                (() => {
                  const host = createdUrl
                    .replace(/^https?:\/\//, '')
                    .split('/')[0]
                  const simplifiedUrl = `visio://${host}/${roomDisplayName.trim()}`
                  return (
                    <>
                      <div
                        className="info-link-header"
                        style={{ marginTop: '8px' }}
                      >
                        <RiGlobalLine size={16} />
                        <span>{t('home.createVisio.simplifiedUrl')}</span>
                        <button
                          className="info-copy-icon"
                          onClick={() =>
                            handleCopy(simplifiedUrl, setCopiedDeep)
                          }
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
                      <span
                        style={{
                          fontSize: '0.75rem',
                          color: 'var(--text-secondary)',
                        }}
                      >
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
        <div
          className="modal-overlay"
          onClick={() => {
            setAliasConflictName('')
            setAliasConflictUrl('')
          }}
        >
          <div
            className="settings-modal"
            onClick={(e) => e.stopPropagation()}
            style={{ maxWidth: 400 }}
          >
            <div className="settings-header">
              <span>
                {t('alias.conflictTitle').replace('{name}', aliasConflictName)}
              </span>
            </div>
            <div
              className="settings-footer"
              style={{
                display: 'flex',
                gap: 8,
                justifyContent: 'flex-end',
                padding: '16px',
              }}
            >
              <button
                className="btn"
                onClick={() => {
                  setAliasConflictName('')
                  setAliasConflictUrl('')
                }}
              >
                {t('alias.conflictCancel')}
              </button>
              <button
                className="btn btn-primary"
                onClick={() => {
                  invoke('add_visio_alias', {
                    name: aliasConflictName,
                    url: aliasConflictUrl,
                  }).catch(() => {})
                  setAliasConflictName('')
                  setAliasConflictUrl('')
                }}
              >
                {t('alias.conflictReplace')}
              </button>
            </div>
          </div>
        </div>
      )}
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

// ---------------------------------------------------------------------------
// Sidebar profile popover
// ---------------------------------------------------------------------------

interface ProfileMenuProps {
  t: (key: string) => string
  onManageAccount: () => void
  onSignOut: () => void
  onClose: () => void
}

function ProfileMenu({
  t,
  onManageAccount,
  onSignOut,
  onClose,
}: ProfileMenuProps) {
  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      const tgt = e.target as Element | null
      if (!tgt) return
      if (!tgt.closest('[data-profile-menu]')) onClose()
    }
    document.addEventListener('mousedown', onDoc)
    return () => document.removeEventListener('mousedown', onDoc)
  }, [onClose])
  return (
    <div
      data-profile-menu
      style={{
        position: 'absolute',
        bottom: 72,
        left: 16,
        width: 240,
        background: 'var(--surface)',
        borderRadius: 'var(--r-card)',
        boxShadow: 'var(--shadow-pop)',
        border: '1px solid var(--border)',
        padding: 6,
        zIndex: 40,
      }}
    >
      <button
        onClick={onManageAccount}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          width: '100%',
          border: 'none',
          background: 'transparent',
          cursor: 'pointer',
          padding: '10px 12px',
          borderRadius: 8,
          fontSize: 13.5,
          color: 'var(--text)',
          fontFamily: 'var(--font-ui)',
          textAlign: 'left',
        }}
      >
        <VIcon name="user" size={16} style={{ color: 'var(--text-2)' }} />
        <span>{t('settings.account.manage')}</span>
      </button>
      <button
        onClick={onSignOut}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          width: '100%',
          border: 'none',
          background: 'transparent',
          cursor: 'pointer',
          padding: '10px 12px',
          borderRadius: 8,
          fontSize: 13.5,
          color: 'var(--danger)',
          fontFamily: 'var(--font-ui)',
          textAlign: 'left',
        }}
      >
        <VIcon name="logout" size={16} />
        <span>{t('settings.signOut')}</span>
      </button>
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
  const [showParticipants, setShowParticipants] = useState(false)
  const [showMicPicker, setShowMicPicker] = useState(false)
  const [showCamPicker, setShowCamPicker] = useState(false)
  // Lobby / waiting room
  const [waitingParticipants, setWaitingParticipants] = useState<
    Array<{ id: string; username: string }>
  >([])
  // lobbyNotification removed — banner now driven by waitingParticipants directly

  // Deep link
  const [deepLinkError, setDeepLinkError] = useState<string | null>(null)
  // Display name (shared between Home and Settings)
  const [displayName, setDisplayName] = useState('')
  // i18n
  const [lang, setLang] = useState(detectSystemLang)
  // Theme
  const [theme, setTheme] = useState('light')
  // OIDC feature flag
  const [oidcEnabled, setOidcEnabled] = useState(true)
  // OIDC auth
  const [isAuthenticated, setIsAuthenticated] = useState(false)
  const [displayNameFromOidc, setDisplayNameFromOidc] = useState('')
  const [emailFromOidc, setEmailFromOidc] = useState('')
  const [authenticatedMeetInstance, setAuthenticatedMeetInstance] = useState('')
  const [meetInstances, setMeetInstances] = useState<string[]>([])
  const pendingOidcRef = useRef<string | null>(null)
  const [bandwidthMode, setBandwidthMode] = useState<string>('full')
  const settingsRef = useRef<Settings | null>(null)

  // ---- Refonte UI desktop ------------------------------------------------
  const [showCreateRoom, setShowCreateRoom] = useState(false)
  const [upcomingMeetings, setUpcomingMeetings] = useState<Meeting[]>([])
  const [imminentMeetingsCount, setImminentMeetingsCount] = useState(0)
  const [homeJoinError, setHomeJoinError] = useState<string | null>(null)
  const [homeJoinPending, setHomeJoinPending] = useState(false)
  const [showProfileMenu, setShowProfileMenu] = useState(false)
  const [visioLinksEnabled, setVisioLinksEnabled] = useState(true)
  const [notificationsEnabled, setNotificationsEnabled] = useState(true)
  const [appBgMode, setAppBgMode] = useState<string>('off')
  // Background images bundled with the app (see tauri.conf.json > bundle.resources:
  // "../../assets/backgrounds/*.jpg": "backgrounds/"). The current set is named
  // 1.jpg..8.jpg under assets/backgrounds/ — IDs are assigned by sorted filename.
  // Each entry holds the u8 id used by Rust's load_background_image/set_background_mode
  // ("image:N") and an absolute file:// path or /public path the UI can render
  // as a thumbnail. The list is populated at startup once the Rust side has
  // registered the images.
  // TODO: replace the static 1..8 list with a Tauri command that lists the
  // unpacked `backgrounds/` resource dir at runtime (avoids touching this file
  // every time a designer drops a new JPEG in assets/backgrounds/).
  const [bgImages, setBgImages] = useState<
    Array<{ id: number; thumbUrl: string }>
  >([])
  const [callStartedMs, setCallStartedMs] = useState<number | null>(null)
  const [infoToast, setInfoToast] = useState<string | null>(null)
  const [layoutMode, setLayoutMode] = useState<string>('grid')
  const [homeMode, setHomeMode] = useState<'main' | 'calendar'>('main')
  const [liveReactions, setLiveReactions] = useState<
    Array<{ id: number; sid: string; emoji: string; ts: number }>
  >([])
  const reactionCounter = useRef(0)
  const [pinnedSid, setPinnedSid] = useState<string | null>(null)

  const showToast = useCallback((text: string, ms = 2400) => {
    setInfoToast(text)
    setTimeout(() => setInfoToast((cur) => (cur === text ? null : cur)), ms)
  }, [])

  const t = useCallback(
    (key: string) => translations[lang]?.[key] ?? translations.en[key] ?? key,
    [lang]
  )

  const handleNavigate = useCallback((k: NavKey) => {
    if (k === 'home') {
      setHomeMode('main')
      setView('home')
      return
    }
    if (k === 'settings') {
      setView('settings')
      return
    }
    if (k === 'calendar') {
      setHomeMode('calendar')
      setView('home')
      invoke('refresh_calendar_now').catch(() => {})
      return
    }
    // 'rooms' | 'recordings' — currently hidden from the sidebar but the
    // type union still allows them; fall through to home.
    setHomeMode('main')
    setView('home')
  }, [])

  // Check OIDC feature flag on mount
  useEffect(() => {
    invoke<boolean>('is_oidc_enabled').then(setOidcEnabled)
  }, [])

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

    // Pre-register bundled background images with Rust so they can be picked
    // by ID via set_background_mode("image:N"). Rust's list_background_images
    // command enumerates the unpacked `backgrounds/` resource dir and returns
    // the available IDs — so dropping a new N.jpg under assets/backgrounds/
    // is enough to make it appear in the picker.
    ;(async () => {
      let ids: number[] = []
      try {
        ids = await invoke<number[]>('list_background_images')
      } catch {
        ids = []
      }
      const loaded: Array<{ id: number; thumbUrl: string }> = []
      for (const id of ids) {
        try {
          const path = await resolveResource(`backgrounds/${id}.jpg`)
          await invoke('load_background_image', { id, jpegPath: path })
          loaded.push({ id, thumbUrl: `/backgrounds/thumbnails/${id}.jpg` })
        } catch {
          // Image present in dir but unreadable — skip silently.
        }
      }
      setBgImages(loaded)
    })()

    // Background mode (sync from Rust SettingsStore)
    invoke<string>('get_background_mode')
      .then((m) => setAppBgMode(m || 'off'))
      .catch(() => {})

    // Initial layout mode
    invoke<string>('get_layout_mode')
      .then((m) => setLayoutMode(m || 'grid'))
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

        // Handle PKCE auth callback: visio://auth-callback?code={...}&state={...}
        if (host === 'auth-callback') {
          const code = parsed.searchParams.get('code')
          const stateParam = parsed.searchParams.get('state')
          const meetInstance = pendingOidcRef.current
          if (code && stateParam && meetInstance) {
            pendingOidcRef.current = null
            invoke<{
              display_name?: string
              email?: string
              meet_instance?: string
            }>('exchange_pkce_code', {
              meetInstance,
              code,
              stateParam,
            })
              .then((result) => {
                setIsAuthenticated(true)
                setAuthenticatedMeetInstance(meetInstance)
                setDisplayNameFromOidc(result.display_name || '')
                setEmailFromOidc(result.email || '')
                if (result.display_name && !displayName.trim()) {
                  setDisplayName(result.display_name)
                }
                invoke<string[]>('get_meet_instances')
                  .then((current) => {
                    if (!current.includes(meetInstance)) {
                      const next = [...current, meetInstance]
                      setMeetInstances(next)
                      invoke('set_meet_instances', { instances: next })
                    } else {
                      setMeetInstances(current)
                    }
                  })
                  .catch(() => {})
              })
              .catch((e) => {
                console.error('PKCE code exchange failed:', e)
              })
          }
          return
        }

        // Handle room deep links: visio://{host}/{slug}[?visio=...]
        const pathSegment = parsed.pathname.replace(/^\//, '')
        if (!host || !pathSegment) return

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
            setDeepLinkError(null)
            return
          }

          // Otherwise try alias resolution
          try {
            const resolved = await invoke<string | null>(
              'resolve_visio_alias',
              { name: pathSegment }
            )
            if (resolved) {
              setView('home')
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
  // Backend retries emission multiple times; we ignore duplicates once connected.
  const autoConnectedRef = useRef(false)
  useEffect(() => {
    const unlisten = listen<{ livekit_url: string; token: string }>(
      'auto-connect',
      async (event) => {
        if (autoConnectedRef.current) return
        autoConnectedRef.current = true
        const { livekit_url, token } = event.payload
        try {
          await invoke('connect_with_token', { livekitUrl: livekit_url, token })
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

  // Dev/debug only: navigate via URL hash between SAFE views (home/settings).
  // Lobby and Call need real state (lobbyRoomUrl, livekit creds, connection
  // state) — jumping into them via hash with empty state crashes connect()
  // and dead-locks the join flow, so they're intentionally excluded.
  useEffect(() => {
    const apply = () => {
      const h = window.location.hash.replace(/^#/, '')
      if (h === 'home' || h === 'settings') {
        setView(h as View)
      }
    }
    apply()
    window.addEventListener('hashchange', apply)
    return () => window.removeEventListener('hashchange', apply)
  }, [])

  // Apply theme to document. We always set an explicit data-theme="light" or
  // "dark" attribute so the legacy App.css [data-theme='dark'] selectors keep
  // matching — even when the user picked 'system' and the OS happens to be
  // dark. The token CSS keys off the same attribute, so both stylesheets stay
  // in lockstep.
  useEffect(() => {
    const root = document.documentElement
    const applyResolved = () => {
      let resolved: 'light' | 'dark'
      if (theme === 'light' || theme === 'dark') {
        resolved = theme
      } else {
        resolved = window.matchMedia?.('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light'
      }
      root.setAttribute('data-theme', resolved)
    }
    applyResolved()
    if (theme === 'system') {
      const mq = window.matchMedia('(prefers-color-scheme: dark)')
      mq.addEventListener('change', applyResolved)
      return () => mq.removeEventListener('change', applyResolved)
    }
  }, [theme])

  const viewRef = useRef(view)
  viewRef.current = view

  // ---- Unified device enumeration (lobby + in-call) -----------------------
  // ONE hook for the whole app — duplicating it (one here, one in LobbyScreen)
  // produced double `audio-devices-changed` subscriptions and racing fallback
  // calls. The lobby-specific "restart mic preview on device change" runs
  // through the fallback callback below, gated on view.
  const onAudioInputFallback = useCallback(() => {
    if (viewRef.current === 'lobby') {
      invoke('stop_mic_preview')
        .catch(() => {})
        .then(() => invoke('start_mic_preview'))
        .catch(() => {})
    }
  }, [])
  const inCallDevices = useDeviceEnumeration({
    onInputFallback: onAudioInputFallback,
  })
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

  // Trigger enumeration lazily when a device picker is first opened or when
  // the user navigates to the Settings screen (so Mic/Camera pickers show
  // real device names instead of empty fallbacks).
  useEffect(() => {
    if (devicesEnumerated) return
    if (!showMicPicker && !showCamPicker && view !== 'settings') return
    enumerateDevices()
  }, [showMicPicker, showCamPicker, view, devicesEnumerated, enumerateDevices])

  // Outside-click closing of the device pickers is handled inside CallScreen
  // (CallPicker uses [data-call-picker]/[data-call-btn] anchors). The legacy
  // App-level handler keyed off .device-picker/.control-chevron and silently
  // killed the new carets on every click.

  // ---- Polling ------------------------------------------------------------
  const poll = useCallback(async () => {
    try {
      const state: string = await invoke('get_connection_state')
      setConnectionState((prev) => {
        // Latch the call start timestamp on the rising edge of 'connected' so
        // the CallScreen timer reflects the actual call duration, not the
        // React mount time.
        if (state === 'connected' && prev !== 'connected') {
          setCallStartedMs(Date.now())
        } else if (state === 'disconnected') {
          setCallStartedMs(null)
        }
        return state
      })

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
        setShowParticipants(false)
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
  // ---- Refonte: chargement des réunions pour la home ---------------------
  useEffect(() => {
    invoke<Meeting[]>('get_upcoming_meetings')
      .then((list) => {
        setUpcomingMeetings(list)
        setImminentMeetingsCount(
          list.filter((m) => isMeetingImminent(m) || isMeetingOngoing(m)).length
        )
      })
      .catch(() => {})
    let off: UnlistenFn | null = null
    listen<Meeting[]>('meetings-updated', (event) => {
      if (event.payload.length > 0) {
        setUpcomingMeetings(event.payload)
        setImminentMeetingsCount(
          event.payload.filter(
            (m) => isMeetingImminent(m) || isMeetingOngoing(m)
          ).length
        )
      }
    }).then((fn) => {
      off = fn
    })
    return () => {
      off?.()
    }
  }, [])

  // ---- Toast on participant join. Skip during the first 2s after the call
  // connects so the initial roster doesn't fire N toasts at once.
  useEffect(() => {
    let off: UnlistenFn | null = null
    listen<{ sid: string; identity: string; name: string }>(
      'participant-joined',
      (event) => {
        if (callStartedMs == null || Date.now() - callStartedMs < 2000) {
          return
        }
        const who = event.payload.name || event.payload.identity || ''
        if (!who) return
        showToast(t('call.participantJoined').replace('{name}', who))
      }
    ).then((fn) => {
      off = fn
    })
    return () => {
      off?.()
    }
  }, [callStartedMs, showToast, t])

  // ---- Listen for reaction-received events at App-level so CallScreen sees
  // them whether it's the active view or not. Each reaction auto-expires
  // after 3.5s. The legacy listener inside the dead CallView never runs.
  useEffect(() => {
    let off: UnlistenFn | null = null
    listen<{ participantSid: string; participantName: string; emoji: string }>(
      'reaction-received',
      (event) => {
        const { participantSid, emoji } = event.payload
        const id = ++reactionCounter.current
        setLiveReactions((prev) => [
          ...prev,
          { id, sid: participantSid, emoji, ts: Date.now() },
        ])
        setTimeout(() => {
          setLiveReactions((prev) => prev.filter((r) => r.id !== id))
        }, 3500)
      }
    ).then((fn) => {
      off = fn
    })
    return () => {
      off?.()
    }
  }, [])

  // ---- Refonte: helpers Home → handleJoin --------------------------------
  const handleNewMeeting = useCallback(() => {
    if (!isAuthenticated && oidcEnabled) {
      // Pas connecté : kicker l'OIDC sur l'instance par défaut.
      const target = meetInstances[0] || authenticatedMeetInstance
      if (target) {
        pendingOidcRef.current = target
        invoke('launch_oidc_browser', { meetInstance: target }).catch((e) =>
          console.error('OIDC launch failed:', e)
        )
        return
      }
    }
    setShowCreateRoom(true)
  }, [isAuthenticated, oidcEnabled, meetInstances, authenticatedMeetInstance])

  const handleJoinByCode = useCallback(
    async (raw: string) => {
      setHomeJoinError(null)
      setHomeJoinPending(true)
      try {
        const trimmed = raw.trim().replace(/\/$/, '')
        const candidates: string[] = []
        if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
          candidates.push(trimmed)
        } else if (trimmed.includes('/')) {
          candidates.push(`https://${trimmed}`)
        } else if (SLUG_REGEX.test(trimmed)) {
          // slug seul → essayer chaque instance connue
          for (const inst of meetInstances) {
            candidates.push(`https://${inst}/${trimmed}`)
          }
          if (authenticatedMeetInstance) {
            candidates.unshift(
              `https://${authenticatedMeetInstance}/${trimmed}`
            )
          }
        } else {
          // alias éventuel
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
          setHomeJoinError(t('home.error.noUrl'))
          return
        }
        for (const url of candidates) {
          const result = await invoke<{
            status: string
            livekit_url?: string
            token?: string
          }>('validate_room', {
            url,
            username: displayName.trim() || null,
          })
          if (result.status === 'valid' || result.status === 'auth_required') {
            await handleJoin(
              url,
              displayName.trim() || null,
              undefined,
              undefined,
              result.livekit_url,
              result.token
            )
            return
          }
        }
        setHomeJoinError(t('home.room.notFound'))
      } catch (e) {
        setHomeJoinError(String(e))
      } finally {
        setHomeJoinPending(false)
      }
    },
    [authenticatedMeetInstance, displayName, meetInstances, t]
  )

  const handleJoin = async (
    meetUrl: string,
    username?: string | null,
    roomId?: string,
    accessLevel?: string,
    livekitUrl?: string,
    livekitToken?: string
  ) => {
    // Extract room display name from query param before storing URL. Only
    // overwrite a name that was pre-seeded (e.g. calendar summary) when the
    // URL actually carries a ?visio= override — otherwise keep what the
    // caller set.
    try {
      const parsed = new URL(
        meetUrl.startsWith('http') ? meetUrl : `https://${meetUrl}`
      )
      const raw = parsed.searchParams.get('visio')
      if (raw) setCurrentRoomDisplayName(decodeURIComponent(raw))
    } catch {
      /* ignore */
    }
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

  // Push-to-talk: hold Space to temporarily unmute. We read micEnabled via a
  // ref so the global listener stays stable and isn't rebound on every mic
  // toggle (the old dep array [view, micEnabled] re-registered the listener
  // every time the mic state flipped).
  const pushToTalkRef = useRef(false)
  const micEnabledRef = useRef(micEnabled)
  micEnabledRef.current = micEnabled
  useEffect(() => {
    if (view !== 'call') return
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.code !== 'Space' || e.repeat) return
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      e.preventDefault()
      if (!micEnabledRef.current && !pushToTalkRef.current) {
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
  }, [view])

  // In-call keyboard shortcuts (Cmd on macOS, Ctrl elsewhere).
  // - Cmd/Ctrl+D : toggle mic
  // - Cmd/Ctrl+E : toggle camera
  // The handlers read latest state via refs so the listener stays stable.
  useEffect(() => {
    if (view !== 'call') return
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey
      if (!mod) return
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      if (e.key === 'd' || e.key === 'D') {
        e.preventDefault()
        const next = !micEnabledRef.current
        setMicEnabled(next)
        invoke('toggle_mic', { enabled: next }).catch(() => {})
      } else if (e.key === 'e' || e.key === 'E') {
        e.preventDefault()
        setCamEnabled((cur) => {
          const next = !cur
          invoke('toggle_camera', { enabled: next }).catch(() => {})
          return next
        })
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [view])

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
    setShowParticipants(false)
    setConnectionState('disconnected')
    setIsHandRaised(false)
    setUnreadCount(0)
    setHandRaisedMap({})
    setActiveSpeakers([])
    setLocalParticipant(null)
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

  // ---- Sign out -----------------------------------------------------------
  const handleSignOut = useCallback(() => {
    const finish = () => {
      setIsAuthenticated(false)
      setAuthenticatedMeetInstance('')
      setDisplayNameFromOidc('')
      setEmailFromOidc('')
      showToast(t('settings.signOut.done'))
      setView('home')
    }
    if (authenticatedMeetInstance) {
      invoke('logout_session', {
        meetUrl: `https://${authenticatedMeetInstance}`,
      })
        .then(finish)
        .catch(finish)
    } else {
      // Anonymous build: just clear local identity state.
      invoke('set_display_name', { name: null }).catch(() => {})
      setDisplayName('')
      finish()
    }
  }, [authenticatedMeetInstance, showToast, t])

  // ---- Render -------------------------------------------------------------
  return (
    <>
      {view === 'call' && bandwidthMode !== 'full' && (
        <div
          className="bandwidth-indicator"
          style={{
            position: 'fixed',
            top: 12,
            left: '50%',
            transform: 'translateX(-50%)',
            zIndex: 50,
            background: 'var(--warn)',
            color: '#fff',
            padding: '6px 14px',
            borderRadius: 'var(--r-card)',
            fontSize: 12,
            fontWeight: 600,
            boxShadow: 'var(--shadow-pop)',
          }}
        >
          {bandwidthMode === 'reduced_video'
            ? t('bandwidth.reducedVideo')
            : t('bandwidth.audioOnly')}
        </div>
      )}
      <main
        style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}
      >
        {view === 'home' && (
          <DeskWindow>
            <DeskSidebar
              active={homeMode === 'calendar' ? 'calendar' : 'home'}
              onNavigate={handleNavigate}
              themeIsDark={isDarkTheme(theme)}
              profile={{
                name: profileDisplayName(displayName, displayNameFromOidc),
                subtitle: profileSubtitle(
                  isAuthenticated,
                  authenticatedMeetInstance,
                  emailFromOidc
                ),
              }}
              labels={{
                home: t('sidebar.home'),
                rooms: t('sidebar.rooms'),
                calendar: t('sidebar.calendar'),
                recordings: t('sidebar.recordings'),
                settings: t('sidebar.settings'),
              }}
              newMeetingSlot={
                isAuthenticated && oidcEnabled ? (
                  <VButton
                    variant="primary"
                    full
                    onClick={handleNewMeeting}
                    icon={<VIcon name="video" size={18} />}
                  >
                    {t('home.newMeetingButton')}
                  </VButton>
                ) : null
              }
              onProfileClick={() => setShowProfileMenu((v) => !v)}
            />
            <HomeScreen
              t={t}
              userFirstName={firstName(
                profileDisplayName(displayName, displayNameFromOidc)
              )}
              instanceHost={
                authenticatedMeetInstance || meetInstances[0] || null
              }
              meetings={upcomingMeetings}
              notifBadge={imminentMeetingsCount}
              showNewMeeting={isAuthenticated && oidcEnabled}
              mode={homeMode}
              meetInstances={meetInstances}
              authenticatedMeetInstance={authenticatedMeetInstance}
              onNewMeeting={handleNewMeeting}
              onJoinByCode={handleJoinByCode}
              onOpenMeeting={(m) => {
                const uname = displayName.trim() || null
                // Pre-seed the displayed title with the calendar summary so
                // the Lobby header shows "COCO 2026" instead of falling back
                // to the generic "Réunion d'équipe" placeholder. handleJoin
                // will overwrite it from a ?visio= query param if present.
                if (m.summary) setCurrentRoomDisplayName(m.summary)
                invoke('set_display_name', { name: uname })
                  .then(() => handleJoin(m.room_url, uname))
                  .catch((e) => setHomeJoinError(String(e)))
              }}
              onOpenCalendar={() => {
                setHomeMode('calendar')
                invoke('refresh_calendar_now').catch(() => {})
              }}
              onRefreshCalendar={() => {
                invoke('refresh_calendar_now').catch(() => {})
                showToast(t('home.upcoming.refreshed'))
              }}
              onOpenNotifications={() => {
                if (imminentMeetingsCount > 0) {
                  showToast(
                    t('home.notifications.summary').replace(
                      '{count}',
                      String(imminentMeetingsCount)
                    )
                  )
                } else {
                  showToast(t('home.notifications.empty'))
                }
              }}
              onOpenInstance={() => setView('settings')}
            />
            {(homeJoinError || deepLinkError) && (
              <div
                style={{
                  position: 'absolute',
                  bottom: 24,
                  left: 274,
                  right: 28,
                  background: 'var(--danger)',
                  color: '#fff',
                  padding: '12px 16px',
                  borderRadius: 'var(--r-card)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: 12,
                  boxShadow: 'var(--shadow-pop)',
                  fontSize: 14,
                  fontWeight: 600,
                }}
              >
                <span>{homeJoinError || deepLinkError}</span>
                <button
                  onClick={() => {
                    setHomeJoinError(null)
                    setDeepLinkError(null)
                  }}
                  style={{
                    background: 'transparent',
                    border: 'none',
                    color: '#fff',
                    cursor: 'pointer',
                    padding: 4,
                    display: 'inline-flex',
                  }}
                  aria-label="dismiss"
                >
                  <RiCloseLine size={16} />
                </button>
              </div>
            )}
            {homeJoinPending && (
              <div
                style={{
                  position: 'absolute',
                  top: 14,
                  right: 18,
                  background: 'var(--surface)',
                  color: 'var(--text-2)',
                  padding: '8px 14px',
                  borderRadius: 'var(--r-card)',
                  fontSize: 13,
                  boxShadow: 'var(--shadow-pop)',
                }}
              >
                {t('home.connecting')}
              </div>
            )}
            {showProfileMenu && (
              <ProfileMenu
                t={t}
                onManageAccount={() => {
                  setShowProfileMenu(false)
                  setView('settings')
                }}
                onSignOut={() => {
                  setShowProfileMenu(false)
                  handleSignOut()
                }}
                onClose={() => setShowProfileMenu(false)}
              />
            )}
          </DeskWindow>
        )}
        {infoToast && (
          <div
            style={{
              position: 'fixed',
              bottom: 24,
              left: '50%',
              transform: 'translateX(-50%)',
              background: 'var(--surface)',
              color: 'var(--text)',
              padding: '10px 18px',
              borderRadius: 'var(--r-card)',
              fontSize: 13.5,
              fontWeight: 500,
              boxShadow: 'var(--shadow-pop)',
              border: '1px solid var(--border)',
              zIndex: 9000,
            }}
            role="status"
          >
            {infoToast}
          </div>
        )}
        {view === 'home' &&
          oidcEnabled &&
          showCreateRoom &&
          authenticatedMeetInstance && (
            <CreateRoomDialog
              meetInstance={authenticatedMeetInstance}
              t={t}
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
                  handleJoin(
                    createdUrl,
                    uname,
                    roomId,
                    accessLevel,
                    livekitUrl,
                    livekitToken
                  )
                } catch (e) {
                  setHomeJoinError(String(e))
                }
              }}
              onCancel={() => setShowCreateRoom(false)}
            />
          )}
        {view === 'lobby' && (
          <DeskWindow>
            <LobbyScreen
              t={t}
              themeIsDark={isDarkTheme(theme)}
              roomTitle={currentRoomDisplayName || ''}
              roomUrl={lobbyRoomUrl}
              livekitUrl={lobbyLivekitUrl}
              livekitToken={lobbyLivekitToken}
              initialUsername={lobbyUsername}
              waitingParticipants={waitingParticipants}
              connectionState={connectionState}
              audioInputs={audioInputs}
              videoInputs={videoInputs}
              selectedAudioInput={selectedAudioInput}
              selectedVideoInput={selectedVideoInput}
              setSelectedAudioInput={handleSelectAudioInput}
              setSelectedVideoInput={handleSelectVideoInput}
              enumerateDevices={enumerateDevices}
              bgMode={appBgMode}
              bgImages={bgImages}
              onSetBgMode={(mode) => {
                setAppBgMode(mode)
                invoke('set_background_mode', { mode })
                  .catch(() => {})
                  .finally(() => {
                    // The camera preview pipeline only re-reads the background
                    // mode on (re)start, so restart it to make the new effect
                    // visible immediately. No-op when the camera is off.
                    invoke('stop_camera_preview')
                      .catch(() => {})
                      .then(() => invoke('start_camera_preview'))
                      .catch(() => {})
                  })
              }}
              onAdmit={(id) => {
                invoke('admit_participant', { participantId: id }).catch(
                  () => {}
                )
                setWaitingParticipants((prev) =>
                  prev.filter((p) => p.id !== id)
                )
              }}
              onDeny={(id) => {
                invoke('deny_participant', { participantId: id }).catch(
                  () => {}
                )
                setWaitingParticipants((prev) =>
                  prev.filter((p) => p.id !== id)
                )
              }}
              onAdmitAll={() => {
                waitingParticipants.forEach((p) => {
                  invoke('admit_participant', { participantId: p.id }).catch(
                    () => {}
                  )
                })
                setWaitingParticipants([])
              }}
              onJoined={async (_username, micOn, camOn, lobbyAudioMode) => {
                const wantMic = micOn && lobbyAudioMode !== 'none'
                if (wantMic) {
                  try {
                    await invoke('toggle_mic', { enabled: true })
                    setMicEnabled(true)
                  } catch (e) {
                    console.error('Failed to enable mic on join:', e)
                  }
                }
                if (camOn) {
                  try {
                    await invoke('toggle_camera', { enabled: true })
                    setCamEnabled(true)
                  } catch (e) {
                    console.error('Failed to enable camera on join:', e)
                  }
                }
                setView('call')
              }}
              onCancel={() => setView('home')}
            />
          </DeskWindow>
        )}
        {view === 'call' && connectionState !== 'waiting_for_host' && (
          <CallScreen
            t={t}
            roomTitle={currentRoomDisplayName}
            participants={participants}
            localParticipant={localParticipant}
            micEnabled={micEnabled}
            camEnabled={camEnabled}
            isHandRaised={isHandRaised}
            videoFrames={videoFrames}
            activeSpeakers={activeSpeakers}
            handRaisedMap={handRaisedMap}
            messages={messages}
            unreadCount={unreadCount}
            encrypted={messages.some((m) => m.encrypted)}
            onSendChat={(text) => {
              handleSendChat(text).catch(() => {})
            }}
            onToggleMic={handleToggleMic}
            onToggleCam={handleToggleCam}
            onHangUp={handleHangUp}
            onToggleHandRaise={handleToggleHandRaise}
            onReact={(emoji) => {
              invoke('send_reaction', { emoji }).catch(() => {})
              // Echo locally — Rust filters self-echoes from the broadcast,
              // so the sender otherwise never sees their own animation.
              if (localParticipant?.sid) {
                const id = ++reactionCounter.current
                setLiveReactions((prev) => [
                  ...prev,
                  {
                    id,
                    sid: localParticipant.sid,
                    emoji,
                    ts: Date.now(),
                  },
                ])
                setTimeout(() => {
                  setLiveReactions((prev) => prev.filter((r) => r.id !== id))
                }, 3500)
              }
            }}
            audioInputs={audioInputs}
            audioOutputs={audioOutputs}
            videoInputs={videoInputs}
            selectedAudioInput={selectedAudioInput}
            selectedAudioOutput={selectedAudioOutput}
            selectedVideoInput={selectedVideoInput}
            onSelectAudioInput={handleSelectAudioInput}
            onSelectAudioOutput={handleSelectAudioOutput}
            onSelectVideoInput={handleSelectVideoInput}
            onShowMicPicker={() => {
              setShowMicPicker((v) => !v)
              setShowCamPicker(false)
            }}
            onShowCamPicker={() => {
              setShowCamPicker((v) => !v)
              setShowMicPicker(false)
            }}
            showMicPicker={showMicPicker}
            showCamPicker={showCamPicker}
            onClosePickers={() => {
              setShowMicPicker(false)
              setShowCamPicker(false)
            }}
            onOpenSettings={() => {
              setView('settings')
            }}
            bgMode={appBgMode}
            bgImages={bgImages}
            onSetBgMode={(mode) => {
              setAppBgMode(mode)
              invoke('set_background_mode', { mode }).catch(() => {})
              // In-call we don't need to restart the preview (the call's
              // own video track is already being processed live by the
              // BlurProcessor), but kicking start_camera_preview is a
              // cheap idempotent re-init if the user toggles bg from a
              // muted-camera state.
              if (camEnabled) {
                invoke('start_camera_preview').catch(() => {})
              }
            }}
            callStartedMs={callStartedMs}
            layoutMode={layoutMode}
            onToggleLayout={() => {
              const next = layoutMode === 'speaker' ? 'grid' : 'speaker'
              setLayoutMode(next)
              invoke('set_layout_mode', { mode: next }).catch(() => {})
            }}
            onTogglePeople={() => setShowParticipants((v) => !v)}
            peopleOpen={showParticipants}
            liveReactions={liveReactions}
            pinnedSid={pinnedSid}
            onTogglePin={(sid) => {
              const next = pinnedSid === sid ? null : sid
              setPinnedSid(next)
              invoke('pin_participant', { sid: next }).catch(() => {})
            }}
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
          <DeskWindow>
            <DeskSidebar
              active="settings"
              onNavigate={handleNavigate}
              themeIsDark={isDarkTheme(theme)}
              profile={{
                name: profileDisplayName(displayName, displayNameFromOidc),
                subtitle: profileSubtitle(
                  isAuthenticated,
                  authenticatedMeetInstance,
                  emailFromOidc
                ),
              }}
              labels={{
                home: t('sidebar.home'),
                rooms: t('sidebar.rooms'),
                calendar: t('sidebar.calendar'),
                recordings: t('sidebar.recordings'),
                settings: t('sidebar.settings'),
              }}
              newMeetingSlot={
                isAuthenticated && oidcEnabled ? (
                  <VButton
                    variant="primary"
                    full
                    onClick={handleNewMeeting}
                    icon={<VIcon name="video" size={18} />}
                  >
                    {t('home.newMeetingButton')}
                  </VButton>
                ) : null
              }
              onProfileClick={() => setShowProfileMenu((v) => !v)}
            />
            <SettingsScreen
              t={t}
              displayName={profileDisplayName(displayName, displayNameFromOidc)}
              email={emailFromOidc}
              isAuthenticated={isAuthenticated}
              oidcEnabled={oidcEnabled}
              onChangeDisplayName={setDisplayName}
              theme={(theme as ThemeChoice) || 'system'}
              onChangeTheme={(next) => {
                setTheme(next)
                invoke('set_theme', { theme: next }).catch(() => {})
              }}
              lang={lang}
              onChangeLanguage={(l) => setLang(l)}
              instanceHost={authenticatedMeetInstance || meetInstances[0] || ''}
              meetInstances={meetInstances}
              onChangeMeetInstances={setMeetInstances}
              audioInputs={audioInputs}
              videoInputs={videoInputs}
              selectedAudioInput={selectedAudioInput}
              selectedVideoInput={selectedVideoInput}
              onSelectAudioInput={handleSelectAudioInput}
              onSelectVideoInput={handleSelectVideoInput}
              backgroundLabel={
                appBgMode === 'off'
                  ? t('settings.row.background.off')
                  : t('settings.row.background.blur')
              }
              onOpenBackground={() => {
                // Toggle blur on/off as a quick action; full background picker
                // remains a follow-up.
                const next = appBgMode === 'off' ? 'blur' : 'off'
                setAppBgMode(next)
                invoke('set_background_mode', { mode: next }).catch(() => {})
              }}
              visioLinksEnabled={visioLinksEnabled}
              onToggleVisioLinks={setVisioLinksEnabled}
              notificationsEnabled={notificationsEnabled}
              onToggleNotifications={setNotificationsEnabled}
              onManageAccount={() => {
                // We're already on the settings screen — keep behaviour
                // identical to a no-op while leaving a hook for future
                // identity-provider deep-linking.
              }}
              onSignOut={handleSignOut}
              onClearLocalData={() => {
                if (window.confirm(t('settings.row.clearData.confirm'))) {
                  invoke('clear_visio_history').catch(() => {})
                  showToast(t('settings.row.clearData.done'))
                }
              }}
              appVersion="0.10.0"
              translations={translations}
            />
            {showProfileMenu && (
              <ProfileMenu
                t={t}
                onManageAccount={() => {
                  setShowProfileMenu(false)
                }}
                onSignOut={() => {
                  setShowProfileMenu(false)
                  handleSignOut()
                }}
                onClose={() => setShowProfileMenu(false)}
              />
            )}
          </DeskWindow>
        )}
      </main>
    </>
  )
}
