import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Icon } from '../components/Icon'
import { IconBtn } from '../components/ui/IconBtn'
import { Tag } from '../components/ui/Tag'
import { Avatar } from '../components/ui/Avatar'
import { Button } from '../components/ui/Button'
import { VisioMark } from '../components/ui/VisioMark'
import { type BgImage } from './CallScreen'
import type {
  NativeAudioDevice,
  NativeVideoDevice,
} from '../useDeviceEnumeration'
import type { Settings } from '../types'

type TFunction = (key: string) => string

export interface LobbyScreenProps {
  t: TFunction
  themeIsDark: boolean
  roomTitle: string
  roomUrl: string
  livekitUrl: string | null
  livekitToken: string | null
  initialUsername: string | null
  waitingParticipants: Array<{ id: string; username: string }>
  /**
   * Rust connection state. When 'waiting_for_host', App.tsx already renders
   * its own WaitingScreen above us — we suppress our own waiting overlay to
   * avoid stacking two of them.
   */
  connectionState: string
  /** Audio/video devices, owned by App.tsx (single hook instance). */
  audioInputs: NativeAudioDevice[]
  videoInputs: NativeVideoDevice[]
  selectedAudioInput: string
  selectedVideoInput: string
  setSelectedAudioInput: (name: string) => void
  setSelectedVideoInput: (uniqueId: string) => void
  enumerateDevices: () => void
  bgMode: string
  bgImages: BgImage[]
  onSetBgMode: (mode: string) => void
  onAdmit: (id: string) => void
  onDeny: (id: string) => void
  onAdmitAll: () => void
  onJoined: (
    finalName: string | null,
    micOn: boolean,
    camOn: boolean,
    audioMode: string
  ) => void
  onCancel: () => void
}

const POLL_INTERVAL_MS = 100
const TIMEOUT_MS = 60_000

/** Strip the protocol and trailing slash for a compact display
 *  (`meet.foo.com/abc-defg-hij`). Falls back to the raw URL on parse error. */
function prettyRoomUrl(url: string): string {
  try {
    const u = new URL(url.startsWith('http') ? url : `https://${url}`)
    return `${u.host}${u.pathname.replace(/\/$/, '')}`
  } catch {
    return url
  }
}

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

interface AudioLevelProps {
  level: number
}
function AudioLevel({ level }: AudioLevelProps) {
  // Clip level into [0..1] and amplify low values — get_mic_level returns
  // an RMS that sits around 0.02-0.15 for normal speech; without scaling
  // we'd never light up more than one bar.
  const norm = Math.min(1, Math.max(0, level))
  const amplified = Math.pow(norm, 0.45)
  const bars = useMemo(() => {
    const base = [5, 7, 10, 13, 15, 13, 10, 7, 5]
    return base.map((b) => Math.round(b * (0.35 + amplified * 1.4)))
  }, [amplified])
  const lit = Math.min(9, Math.max(1, Math.round(amplified * 9)))
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 3, height: 18 }}>
      {bars.map((h, i) => (
        <div
          key={i}
          style={{
            width: 3,
            height: Math.max(3, h),
            borderRadius: 2,
            background: i < lit ? '#fff' : 'rgba(255,255,255,0.3)',
            transition: 'background .08s linear',
          }}
        />
      ))}
    </div>
  )
}

interface DDeviceProps {
  icon: string
  label: string
  value: string
  onClick?: () => void
}
function DDevice({ icon, label, value, onClick }: DDeviceProps) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        padding: '11px 14px',
        borderRadius: 'var(--r-input)',
        background: 'var(--surface-2)',
        border: 'none',
        flex: 1,
        minWidth: 0,
        cursor: onClick ? 'pointer' : 'default',
        textAlign: 'left',
        fontFamily: 'var(--font-ui)',
      }}
    >
      <Icon
        name={icon}
        size={17}
        style={{ color: 'var(--text-2)', flexShrink: 0 }}
      />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontSize: 10.5,
            fontWeight: 700,
            letterSpacing: '0.05em',
            textTransform: 'uppercase',
            color: 'var(--text-3)',
          }}
        >
          {label}
        </div>
        <div
          style={{
            fontSize: 13.5,
            fontWeight: 600,
            color: 'var(--text)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {value}
        </div>
      </div>
      {onClick && (
        <Icon
          name="chevronDown"
          size={15}
          style={{ color: 'var(--text-3)', flexShrink: 0 }}
        />
      )}
    </button>
  )
}

export function LobbyScreen({
  t,
  themeIsDark,
  roomTitle,
  roomUrl,
  livekitUrl,
  livekitToken,
  initialUsername,
  waitingParticipants,
  connectionState,
  audioInputs: inputDevices,
  videoInputs: videoDevices,
  selectedAudioInput,
  selectedVideoInput,
  setSelectedAudioInput,
  setSelectedVideoInput,
  enumerateDevices,
  bgMode,
  bgImages,
  onSetBgMode,
  onAdmit,
  onDeny,
  onAdmitAll,
  onJoined,
  onCancel,
}: LobbyScreenProps) {
  const [displayName, setDisplayName] = useState(initialUsername ?? '')
  const [isMicOn, setIsMicOn] = useState(true)
  const [isCameraOn, setIsCameraOn] = useState(false)
  const [audioMode, setAudioMode] = useState<'computer' | 'none'>('computer')
  const [previewFrame, setPreviewFrame] = useState<string | null>(null)
  const [micLevel, setMicLevel] = useState(0)
  const [deviceError, setDeviceError] = useState<string | null>(null)
  const [waitingState, setWaitingState] = useState<
    'idle' | 'waiting' | 'denied' | 'timeout'
  >('idle')
  const [showMicMenu, setShowMicMenu] = useState(false)
  const [showCamMenu, setShowCamMenu] = useState(false)

  const micPollRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const unlistenRef = useRef<UnlistenFn | null>(null)
  const joiningRef = useRef(false)
  const mountedRef = useRef(true)

  // ---- Mount: load settings, devices, video frame stream ------------------
  useEffect(() => {
    let cancelled = false

    invoke<Settings>('get_settings')
      .then((s) => {
        if (cancelled) return
        if (s.display_name && !initialUsername) setDisplayName(s.display_name)
        setIsMicOn(s.mic_enabled_on_join ?? true)
        setIsCameraOn(s.camera_enabled_on_join ?? false)
        if (s.audio_mode === 'none') setAudioMode('none')
      })
      .catch(() => {})

    enumerateDevices()

    listen<{ track_sid: string; data: string; width: number; height: number }>(
      'video-frame',
      (event) => {
        if (event.payload.track_sid === 'local-camera') {
          setPreviewFrame(event.payload.data)
        }
      }
    ).then((unlisten) => {
      // Component may have unmounted while listen() was still pending — call
      // unlisten right away so the bridge subscription doesn't leak.
      if (cancelled) {
        unlisten()
        return
      }
      unlistenRef.current = unlisten
    })

    return () => {
      cancelled = true
      mountedRef.current = false
      unlistenRef.current?.()
      unlistenRef.current = null
      if (micPollRef.current) clearInterval(micPollRef.current)
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
      if (!joiningRef.current) {
        invoke('stop_camera_preview').catch(() => {})
        invoke('stop_mic_preview').catch(() => {})
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // ---- Camera preview on/off ----------------------------------------------
  useEffect(() => {
    if (isCameraOn) {
      invoke('start_camera_preview').catch((e) => {
        setDeviceError(t('lobby.error.camera'))
        setIsCameraOn(false)
        console.error('start_camera_preview failed:', e)
      })
    } else {
      invoke('stop_camera_preview').catch(() => {})
      setPreviewFrame(null)
    }
  }, [isCameraOn, t])

  // ---- Mic preview on/off + audioMode -------------------------------------
  useEffect(() => {
    if (isMicOn && audioMode === 'computer') {
      invoke('start_mic_preview').catch((e) => {
        setDeviceError(t('lobby.error.mic'))
        setIsMicOn(false)
        console.error('start_mic_preview failed:', e)
      })
      micPollRef.current = setInterval(async () => {
        try {
          const level = await invoke<number>('get_mic_level')
          setMicLevel(level)
        } catch {
          setMicLevel(0)
        }
      }, POLL_INTERVAL_MS)
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

  // ---- Handlers -----------------------------------------------------------
  const handleSelectInput = useCallback(
    async (name: string) => {
      setSelectedAudioInput(name)
      try {
        await invoke('select_audio_input', { deviceName: name })
      } catch {
        /* ignore */
      }
      if (isMicOn && audioMode === 'computer') {
        try {
          await invoke('stop_mic_preview')
        } catch {
          /* ignore */
        }
        try {
          await invoke('start_mic_preview')
        } catch {
          /* ignore */
        }
      }
    },
    [audioMode, isMicOn, setSelectedAudioInput]
  )

  const handleSelectCamera = useCallback(
    async (uniqueId: string) => {
      setSelectedVideoInput(uniqueId)
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
    },
    [isCameraOn, setSelectedVideoInput]
  )

  const handleJoinNow = useCallback(
    async (withCamera = isCameraOn) => {
      const finalName = displayName.trim() || null

      await invoke('set_display_name', { name: finalName }).catch(() => {})
      await invoke('set_mic_enabled_on_join', { enabled: isMicOn }).catch(
        () => {}
      )
      await invoke('set_camera_enabled_on_join', { enabled: withCamera }).catch(
        () => {}
      )
      await invoke('set_audio_mode', { mode: audioMode }).catch(() => {})

      await invoke('stop_camera_preview').catch(() => {})
      await invoke('stop_mic_preview').catch(() => {})

      // LiveKit credentials → connect directly (creator path)
      if (livekitUrl && livekitToken) {
        try {
          await invoke('connect_with_token', {
            livekitUrl,
            token: livekitToken,
          })
          invoke('add_visio_to_history', {
            url: roomUrl,
            displayName: null,
          }).catch(() => {})
          joiningRef.current = true
          onJoined(finalName, isMicOn, withCamera, audioMode)
        } catch (e) {
          console.error('connect_with_token failed:', e)
          setWaitingState('idle')
        }
        return
      }

      setWaitingState('waiting')

      timeoutRef.current = setTimeout(() => {
        setWaitingState((prev) => (prev === 'waiting' ? 'timeout' : prev))
      }, TIMEOUT_MS)

      // Register both listeners in parallel. If the component unmounts
      // while listen() is still pending, mountedRef flips to false and we
      // call unlisten immediately to avoid a leaked IPC subscription.
      const [unlistenDenied, unlistenState] = await Promise.all([
        listen<string>('lobby-denied', () => {
          if (timeoutRef.current) clearTimeout(timeoutRef.current)
          setWaitingState('denied')
        }),
        listen<string>('connection-state-changed', (event) => {
          if (event.payload === 'connected') {
            if (timeoutRef.current) clearTimeout(timeoutRef.current)
            joiningRef.current = true
            onJoined(finalName, isMicOn, withCamera, audioMode)
          }
        }),
      ])
      if (!mountedRef.current) {
        unlistenDenied()
        unlistenState()
      } else {
        chainUnlisten(unlistenRef, unlistenDenied)
        chainUnlisten(unlistenRef, unlistenState)
      }

      try {
        await invoke('connect', { meetUrl: roomUrl, username: finalName })
      } catch {
        if (timeoutRef.current) clearTimeout(timeoutRef.current)
        setWaitingState('idle')
      }
    },
    [
      audioMode,
      displayName,
      isCameraOn,
      isMicOn,
      livekitToken,
      livekitUrl,
      onJoined,
      roomUrl,
    ]
  )

  const selectedMicLabel = useMemo(() => {
    const d = inputDevices.find((x) => x.name === selectedAudioInput)
    return d?.name || inputDevices[0]?.name || '—'
  }, [inputDevices, selectedAudioInput])

  const selectedCamLabel = useMemo(() => {
    const d = videoDevices.find((x) => x.unique_id === selectedVideoInput)
    return d?.name || videoDevices[0]?.name || '—'
  }, [videoDevices, selectedVideoInput])

  // ---- Waiting state overlay ----------------------------------------------
  // When the Rust side is in 'waiting_for_host', App.tsx renders its own
  // WaitingScreen on top of us — skip our local overlay to avoid stacking
  // two of them. We still keep the lobby-denied / timeout listeners alive
  // through `waitingState`, so a denial that fires after App.tsx exits the
  // waiting_for_host state still surfaces correctly here.
  if (waitingState !== 'idle' && connectionState !== 'waiting_for_host') {
    return (
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background: 'var(--bg)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          flexDirection: 'column',
          gap: 16,
          fontFamily: 'var(--font-ui)',
          color: 'var(--text)',
          zIndex: 10,
        }}
      >
        {waitingState === 'waiting' && (
          <>
            <span
              aria-hidden
              style={{
                width: 36,
                height: 36,
                borderRadius: '50%',
                border:
                  '3px solid color-mix(in oklab, var(--accent) 25%, transparent)',
                borderTopColor: 'var(--accent)',
                animation: 'vspin .8s linear infinite',
              }}
            />
            <div style={{ fontSize: 16, fontWeight: 600 }}>
              {t('prejoin.waitingForApproval')}
            </div>
          </>
        )}
        {waitingState === 'denied' && (
          <div
            style={{ fontSize: 16, fontWeight: 600, color: 'var(--danger)' }}
          >
            {t('prejoin.denied')}
          </div>
        )}
        {waitingState === 'timeout' && (
          <div style={{ fontSize: 16, fontWeight: 600, color: 'var(--warn)' }}>
            {t('prejoin.timeout')}
          </div>
        )}
        <Button variant="ghost" onClick={onCancel}>
          {t('settings.cancel')}
        </Button>
      </div>
    )
  }

  // ---- Main lobby UI ------------------------------------------------------
  return (
    <div
      style={{
        flex: 1,
        minWidth: 0,
        background: 'var(--bg)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          borderBottom: '1px solid var(--border)',
          display: 'flex',
          alignItems: 'center',
          gap: 14,
          padding: '0 20px',
        }}
      >
        <VisioMark size={24} dark={themeIsDark} />
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            minWidth: 0,
            gap: 1,
          }}
        >
          <span
            style={{
              fontWeight: 600,
              fontSize: 15,
              letterSpacing: '-0.02em',
              color: 'var(--text)',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {roomTitle || t('lobby.title')}
          </span>
          {roomUrl && (
            <span
              className="v-mono"
              style={{
                fontSize: 11.5,
                color: 'var(--text-3)',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
              title={roomUrl}
            >
              {prettyRoomUrl(roomUrl)}
            </span>
          )}
        </div>
        <div style={{ flex: 1 }} />
        <IconBtn
          name="x"
          dim={36}
          size={17}
          variant="soft"
          tint="var(--text-3)"
          onClick={onCancel}
          ariaLabel={t('settings.cancel')}
        />
      </div>

      <div
        className="v-scroll"
        style={{
          flex: 1,
          minWidth: 0,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          padding: '0 44px 24px',
          overflowY: 'auto',
        }}
      >
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1.05fr',
            gap: 24,
            width: '100%',
            maxWidth: 1200,
            alignItems: 'start',
          }}
        >
          {/* preview + devices */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            {deviceError && (
              <div
                role="alert"
                style={{
                  background: 'var(--danger)',
                  color: '#fff',
                  padding: '10px 14px',
                  borderRadius: 'var(--r-card)',
                  fontSize: 13.5,
                  fontWeight: 600,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: 12,
                }}
              >
                <span>{deviceError}</span>
                <button
                  onClick={() => setDeviceError(null)}
                  aria-label={t('lobby.error.dismiss')}
                  style={{
                    background: 'transparent',
                    border: 'none',
                    color: '#fff',
                    cursor: 'pointer',
                    padding: 0,
                    display: 'inline-flex',
                  }}
                >
                  <Icon name="x" size={16} />
                </button>
              </div>
            )}
            <div
              style={{
                position: 'relative',
                aspectRatio: '16/10',
                borderRadius: 'var(--r-card)',
                overflow: 'hidden',
                background: 'linear-gradient(150deg, #2a3550, #0c1018)',
              }}
            >
              {previewFrame && isCameraOn ? (
                <img
                  src={`data:image/jpeg;base64,${previewFrame}`}
                  alt=""
                  style={{
                    width: '100%',
                    height: '100%',
                    objectFit: 'cover',
                    display: 'block',
                  }}
                />
              ) : (
                <div
                  style={{
                    position: 'absolute',
                    inset: 0,
                    background:
                      'radial-gradient(120% 80% at 70% 20%, rgba(255,255,255,0.16), transparent 55%)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}
                >
                  <Avatar name={displayName || 'Visio'} size={84} />
                </div>
              )}
              <div style={{ position: 'absolute', top: 14, left: 14 }}>
                <Tag tone="neutral" dot>
                  {t('lobby.preview')}
                </Tag>
              </div>
              {videoDevices.length > 1 && (
                <div
                  style={{
                    position: 'absolute',
                    top: 14,
                    right: 14,
                    display: 'flex',
                    gap: 8,
                  }}
                >
                  <IconBtn
                    name="camFlip"
                    dim={36}
                    size={16}
                    variant="glass"
                    tint="#fff"
                    onClick={() => {
                      const idx = videoDevices.findIndex(
                        (d) => d.unique_id === selectedVideoInput
                      )
                      const nextDev =
                        videoDevices[(idx + 1) % videoDevices.length]
                      if (nextDev) setSelectedVideoInput(nextDev.unique_id)
                    }}
                    ariaLabel={t('call.switchCamera')}
                  />
                </div>
              )}
              <div
                style={{
                  position: 'absolute',
                  bottom: 14,
                  left: 14,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  background: 'rgba(8,10,14,0.45)',
                  backdropFilter: 'blur(6px)',
                  padding: '7px 12px',
                  borderRadius: 999,
                }}
              >
                <Icon name="mic" size={14} color="#fff" />
                <AudioLevel level={micLevel} />
              </div>
              <div
                style={{
                  position: 'absolute',
                  bottom: 14,
                  left: 0,
                  right: 0,
                  display: 'flex',
                  justifyContent: 'center',
                  gap: 12,
                }}
              >
                <IconBtn
                  name={isMicOn ? 'mic' : 'micOff'}
                  dim={46}
                  size={20}
                  variant={isMicOn ? 'glass' : 'off'}
                  tint="#fff"
                  onClick={() => setIsMicOn((v) => !v)}
                  ariaLabel={isMicOn ? t('control.mute') : t('control.unmute')}
                />
                <IconBtn
                  name={isCameraOn ? 'video' : 'videoOff'}
                  dim={46}
                  size={20}
                  variant={isCameraOn ? 'glass' : 'off'}
                  tint="#fff"
                  onClick={() => setIsCameraOn((v) => !v)}
                  ariaLabel={
                    isCameraOn ? t('control.camOff') : t('control.camOn')
                  }
                />
              </div>
            </div>
          </div>

          {/* right col: ready + device pickers + bg picker + waiting */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div>
              <div className="v-h1" style={{ fontSize: 25 }}>
                {t('lobby.ready')}
              </div>
              {waitingParticipants.length > 0 && (
                <div className="v-sub" style={{ marginTop: 4 }}>
                  {t('lobby.waitingCount').replace(
                    '{count}',
                    String(waitingParticipants.length)
                  )}
                </div>
              )}
            </div>

            <div style={{ display: 'flex', gap: 12, position: 'relative' }}>
              <div style={{ position: 'relative', flex: 1, minWidth: 0 }}>
                <DDevice
                  icon="mic"
                  label={t('lobby.device.microphone')}
                  value={selectedMicLabel}
                  onClick={() => {
                    setShowMicMenu((v) => !v)
                    setShowCamMenu(false)
                  }}
                />
                {showMicMenu && (
                  <PickerMenu
                    title={t('devicePicker.audio.mic')}
                    items={inputDevices.map((d) => ({
                      key: d.name,
                      label: d.name,
                      active: selectedAudioInput === d.name,
                    }))}
                    onClose={() => setShowMicMenu(false)}
                    onPick={(k) => {
                      handleSelectInput(k)
                      setShowMicMenu(false)
                    }}
                  />
                )}
              </div>
              <div style={{ position: 'relative', flex: 1, minWidth: 0 }}>
                <DDevice
                  icon="video"
                  label={t('lobby.device.camera')}
                  value={selectedCamLabel}
                  onClick={() => {
                    setShowCamMenu((v) => !v)
                    setShowMicMenu(false)
                  }}
                />
                {showCamMenu && (
                  <PickerMenu
                    title={t('devicePicker.video.camera')}
                    items={videoDevices.map((d) => ({
                      key: d.unique_id,
                      label: d.name,
                      active: selectedVideoInput === d.unique_id,
                    }))}
                    onClose={() => setShowCamMenu(false)}
                    onPick={(k) => {
                      handleSelectCamera(k)
                      setShowCamMenu(false)
                    }}
                  />
                )}
              </div>
            </div>

            {/* Always-visible background picker — right of the preview so it
                grows with the window width instead of being cropped at the
                default 1080×680 size. */}
            <BgPanel
              t={t}
              bgMode={bgMode}
              bgImages={bgImages}
              onSetBgMode={onSetBgMode}
            />

            {waitingParticipants.length > 0 && (
              <div
                className="v-card"
                style={{ padding: '14px 16px', marginTop: 2 }}
              >
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    marginBottom: 8,
                  }}
                >
                  <div
                    style={{ display: 'flex', alignItems: 'center', gap: 8 }}
                  >
                    <span style={{ fontSize: 14, fontWeight: 700 }}>
                      {t('lobby.waitingRoom')}
                    </span>
                    <span
                      style={{
                        minWidth: 20,
                        height: 20,
                        padding: '0 6px',
                        borderRadius: 10,
                        background: 'var(--warn)',
                        color: '#fff',
                        fontSize: 12,
                        fontWeight: 700,
                        display: 'inline-flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                      }}
                    >
                      {waitingParticipants.length}
                    </span>
                  </div>
                  <button
                    onClick={onAdmitAll}
                    style={{
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      color: 'var(--accent)',
                      fontWeight: 600,
                      fontSize: 13,
                      fontFamily: 'var(--font-ui)',
                      padding: 0,
                    }}
                  >
                    {t('lobby.admitAll')}
                  </button>
                </div>
                {waitingParticipants.map((p, i) => (
                  <div
                    key={p.id}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 11,
                      padding: '9px 0',
                      borderBottom:
                        i < waitingParticipants.length - 1
                          ? '1px solid var(--hair)'
                          : 'none',
                    }}
                  >
                    <Avatar name={p.username} size={34} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 14, fontWeight: 600 }}>
                        {p.username || t('lobby.guestPlaceholder')}
                      </div>
                      <div style={{ fontSize: 12, color: 'var(--text-3)' }}>
                        {t('lobby.askingToJoin')}
                      </div>
                    </div>
                    <IconBtn
                      name="x"
                      dim={32}
                      size={15}
                      variant="soft"
                      tint="var(--text-3)"
                      onClick={() => onDeny(p.id)}
                      ariaLabel={t('lobby.deny')}
                    />
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={() => onAdmit(p.id)}
                    >
                      {t('lobby.admit')}
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Sticky bottom join bar. Outside the scrollable body so the Join
          button stays in reach even when the right column (waiting room +
          long device names + effects panel) overflows. */}
      <div
        style={{
          flexShrink: 0,
          borderTop: '1px solid var(--border)',
          padding: '14px 44px',
          background: 'var(--surface)',
          display: 'flex',
          justifyContent: 'center',
        }}
      >
        <Button
          variant="primary"
          onClick={() => handleJoinNow(isCameraOn)}
          style={{ height: 48, minWidth: 280 }}
        >
          {t('lobby.joinNow')}
        </Button>
      </div>
    </div>
  )
}

// Inline background-effects panel rendered in the Lobby right column.
// Same content as CallScreen's BgPicker popover, but as a regular card so
// it doesn't overlay the camera preview.
interface BgPanelProps {
  t: TFunction
  bgMode: string
  bgImages: BgImage[]
  onSetBgMode: (mode: string) => void
}
function BgPanel({ t, bgMode, bgImages, onSetBgMode }: BgPanelProps) {
  const effects: Array<[string, string, string]> = [
    ['off', t('bg.off'), 'noBg'],
    ['blur', t('bg.blurStrong'), 'blurStrong'],
    ['blur-light', t('bg.blurLight'), 'blurLight'],
  ]
  return (
    <div
      className="v-card"
      style={{
        padding: '12px 14px',
        display: 'flex',
        flexDirection: 'column',
        gap: 10,
      }}
    >
      <div
        className="v-eyebrow"
        style={{ display: 'flex', alignItems: 'center', gap: 8 }}
      >
        <Icon name="sparkle" size={14} />
        <span>{t('bg.title')}</span>
      </div>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fill, minmax(96px, 1fr))',
          gap: 8,
        }}
      >
        {effects.map(([mode, label]) => {
          const selected = bgMode === mode
          return (
            <button
              key={mode}
              onClick={() => onSetBgMode(mode)}
              style={{
                aspectRatio: '16/10',
                border: selected
                  ? '2px solid var(--accent)'
                  : '2px solid var(--border)',
                borderRadius: 8,
                padding: 0,
                overflow: 'hidden',
                background:
                  mode === 'off'
                    ? 'repeating-linear-gradient(45deg, var(--surface-2) 0 6px, var(--surface) 6px 12px)'
                    : mode === 'blur'
                      ? 'linear-gradient(135deg, #c5cad6, #8a93a8)'
                      : 'linear-gradient(135deg, #e8eaf0, #b9bfcc)',
                cursor: 'pointer',
                position: 'relative',
                display: 'flex',
                alignItems: 'flex-end',
                justifyContent: 'center',
              }}
            >
              <span
                style={{
                  width: '100%',
                  textAlign: 'center',
                  fontSize: 11.5,
                  fontWeight: 600,
                  color: '#222',
                  background: 'rgba(255,255,255,0.85)',
                  padding: '3px 4px',
                  fontFamily: 'var(--font-ui)',
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
              >
                {label}
              </span>
            </button>
          )
        })}
        {bgImages.map((img) => {
          const mode = `image:${img.id}`
          const selected = bgMode === mode
          return (
            <button
              key={img.id}
              onClick={() => onSetBgMode(mode)}
              aria-label={t('bg.imageAlt').replace('{n}', String(img.id))}
              style={{
                aspectRatio: '16/10',
                border: selected
                  ? '2px solid var(--accent)'
                  : '2px solid transparent',
                borderRadius: 8,
                padding: 0,
                overflow: 'hidden',
                background: 'var(--surface-2)',
                cursor: 'pointer',
              }}
            >
              <img
                src={img.thumbUrl}
                alt=""
                style={{
                  width: '100%',
                  height: '100%',
                  objectFit: 'cover',
                  display: 'block',
                }}
              />
            </button>
          )
        })}
      </div>
    </div>
  )
}

interface PickerMenuProps {
  title: string
  items: Array<{ key: string; label: string; active: boolean }>
  onPick: (key: string) => void
  onClose: () => void
}
function PickerMenu({ title, items, onPick, onClose }: PickerMenuProps) {
  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      const tgt = e.target as Element
      if (!tgt.closest('[data-picker-menu]')) onClose()
    }
    document.addEventListener('mousedown', onDoc)
    return () => document.removeEventListener('mousedown', onDoc)
  }, [onClose])
  return (
    <div
      data-picker-menu
      style={{
        position: 'absolute',
        top: 'calc(100% + 6px)',
        left: 0,
        right: 0,
        background: 'var(--surface)',
        borderRadius: 'var(--r-card)',
        boxShadow: 'var(--shadow-pop)',
        padding: 8,
        zIndex: 20,
        maxHeight: 280,
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
    </div>
  )
}

export default LobbyScreen
