import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '../components/Icon'
import { IconBtn } from '../components/ui/IconBtn'
import { Tag } from '../components/ui/Tag'
import { Avatar } from '../components/ui/Avatar'
import { VisioMark } from '../components/ui/VisioMark'
import type { Participant, ChatMessage } from '../types'
import type {
  NativeAudioDevice,
  NativeVideoDevice,
} from '../useDeviceEnumeration'

type TFunction = (key: string) => string

export interface CallScreenProps {
  t: TFunction
  roomTitle: string | null
  participants: Participant[]
  localParticipant: Participant | null
  micEnabled: boolean
  camEnabled: boolean
  isHandRaised: boolean
  videoFrames: Map<string, string>
  activeSpeakers: string[]
  handRaisedMap: Record<string, number>
  messages: ChatMessage[]
  unreadCount: number
  encrypted: boolean
  onSendChat: (text: string) => void
  onToggleMic: () => void
  onToggleCam: () => void
  onHangUp: () => void
  onToggleHandRaise: () => void
  onToggleScreenShare: () => void
  onReact: (emoji: string) => void
  audioInputs: NativeAudioDevice[]
  audioOutputs: NativeAudioDevice[]
  videoInputs: NativeVideoDevice[]
  selectedAudioInput: string
  selectedAudioOutput: string
  selectedVideoInput: string
  onSelectAudioInput: (name: string) => void
  onSelectAudioOutput: (name: string) => void
  onSelectVideoInput: (uniqueId: string) => void
  onShowMicPicker: () => void
  onShowCamPicker: () => void
  showMicPicker: boolean
  showCamPicker: boolean
  onClosePickers: () => void
  onOpenSettings: () => void
  bgMode: string
  onSetBgMode: (mode: string) => void
  /** Unix ms of the rising edge of connectionState === 'connected'. */
  callStartedMs: number | null
  layoutMode: string
  onToggleLayout: () => void
  onTogglePeople: () => void
  peopleOpen: boolean
}

const TONE_PALETTE = [
  '#3a5bd9',
  '#0f9d6b',
  '#7a55d6',
  '#e26039',
  '#d99413',
  '#3a7bd5',
]
function toneFor(seed: string): string {
  let s = 0
  for (const c of seed) s += c.charCodeAt(0)
  return TONE_PALETTE[s % TONE_PALETTE.length]
}

interface DisplayItem {
  key: string
  participant: Participant
  tone: string
  isLocal: boolean
  source: 'camera' | 'screen_share'
  trackSid: string | null
  speaking: boolean
  hand: boolean
}

function buildDisplayItems(
  local: Participant | null,
  participants: Participant[],
  activeSpeakers: string[],
  handRaisedMap: Record<string, number>,
  t: TFunction
): DisplayItem[] {
  const items: DisplayItem[] = []
  const speak = new Set(activeSpeakers)
  if (local) {
    items.push({
      key: `${local.sid}-cam`,
      participant: { ...local, name: local.name || t('call.you') },
      tone: toneFor(local.identity || 'me'),
      isLocal: true,
      source: 'camera',
      trackSid: local.video_track_sid,
      speaking: speak.has(local.sid),
      hand: !!handRaisedMap[local.sid],
    })
  }
  for (const p of participants) {
    items.push({
      key: `${p.sid}-cam`,
      participant: p,
      tone: toneFor(p.identity || p.sid),
      isLocal: false,
      source: 'camera',
      trackSid: p.video_track_sid,
      speaking: speak.has(p.sid),
      hand: !!handRaisedMap[p.sid],
    })
    if (p.has_screen_share && p.screen_share_track_sid) {
      items.push({
        key: `${p.sid}-screen`,
        participant: p,
        tone: '#1a1d24',
        isLocal: false,
        source: 'screen_share',
        trackSid: p.screen_share_track_sid,
        speaking: false,
        hand: false,
      })
    }
  }
  return items
}

function fmtElapsed(startMs: number, nowMs: number): string {
  const sec = Math.max(0, Math.floor((nowMs - startMs) / 1000))
  const h = Math.floor(sec / 3600)
  const m = Math.floor((sec % 3600) / 60)
  const s = sec % 60
  if (h > 0)
    return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
}

function gridCols(n: number): string {
  if (n <= 1) return '1fr'
  if (n <= 4) return '1fr 1fr'
  if (n <= 9) return '1fr 1fr 1fr'
  return '1fr 1fr 1fr 1fr'
}
function gridRows(n: number): string {
  if (n <= 1) return '1fr'
  if (n <= 2) return '1fr'
  if (n <= 4) return '1fr 1fr'
  if (n <= 6) return '1fr 1fr'
  if (n <= 9) return '1fr 1fr 1fr'
  return '1fr 1fr 1fr'
}

interface CallTileProps {
  item: DisplayItem
  frame: string | null
  big?: boolean
}
function CallTile({ item, frame, big }: CallTileProps) {
  const { participant: p, tone, isLocal, source } = item
  const name =
    isLocal && p.name === 'You' ? p.name : p.name || p.identity || '—'
  const hasVideo =
    (source === 'camera' && p.has_video && !!frame) ||
    (source === 'screen_share' && !!frame)

  return (
    <div
      style={{
        position: 'relative',
        width: '100%',
        height: '100%',
        borderRadius: 14,
        overflow: 'hidden',
        background: hasVideo
          ? '#0b0d12'
          : `linear-gradient(150deg, ${tone}, color-mix(in oklab, ${tone} 45%, #05060a))`,
        boxShadow: item.speaking
          ? '0 0 0 3px var(--accent)'
          : 'inset 0 0 0 1px rgba(255,255,255,0.06)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      {hasVideo ? (
        <img
          src={`data:image/jpeg;base64,${frame}`}
          alt=""
          style={{
            width: '100%',
            height: '100%',
            objectFit: 'cover',
            display: 'block',
          }}
        />
      ) : (
        <Avatar name={name} size={big ? 84 : 56} />
      )}
      <div
        style={{
          position: 'absolute',
          left: 10,
          bottom: 10,
          display: 'flex',
          alignItems: 'center',
          gap: 5,
          background: 'rgba(8,10,14,0.55)',
          backdropFilter: 'blur(6px)',
          padding: '4px 9px 4px 7px',
          borderRadius: 8,
        }}
      >
        <Icon
          name={p.is_muted ? 'micOff' : 'mic'}
          size={13}
          color={p.is_muted ? '#ff6b6f' : '#fff'}
        />
        <span
          style={{
            color: '#fff',
            fontSize: 12,
            fontWeight: 600,
            maxWidth: big ? 200 : 110,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {source === 'screen_share' ? `${name} — Écran` : name}
        </span>
      </div>
      {item.speaking && (
        <div style={{ position: 'absolute', top: 10, right: 10 }}>
          <Tag tone="live" dot>
            Parle
          </Tag>
        </div>
      )}
      {item.hand && (
        <div
          style={{
            position: 'absolute',
            top: 10,
            left: 10,
            width: 26,
            height: 26,
            borderRadius: 8,
            background: 'var(--warn)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <Icon name="hand" size={15} color="#fff" />
        </div>
      )}
    </div>
  )
}

interface ChatPanelProps {
  t: TFunction
  messages: ChatMessage[]
  localSid: string | null
  onSend: (text: string) => void
  onClose: () => void
}
function ChatPanel({ t, messages, localSid, onSend, onClose }: ChatPanelProps) {
  const [draft, setDraft] = useState('')
  const scrollerRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const el = scrollerRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [messages.length])

  const submit = () => {
    const txt = draft.trim()
    if (!txt) return
    onSend(txt)
    setDraft('')
  }

  return (
    <div
      style={{
        width: 340,
        flexShrink: 0,
        background: 'rgba(20,22,28,0.7)',
        backdropFilter: 'blur(20px)',
        borderLeft: '1px solid rgba(255,255,255,0.08)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        style={{
          height: 52,
          flexShrink: 0,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 18px',
          borderBottom: '1px solid rgba(255,255,255,0.08)',
        }}
      >
        <span style={{ color: '#fff', fontSize: 15, fontWeight: 700 }}>
          {t('call.discussion')}
        </span>
        <button
          onClick={onClose}
          style={{
            background: 'transparent',
            border: 'none',
            cursor: 'pointer',
            color: 'rgba(255,255,255,0.6)',
            padding: 4,
            display: 'inline-flex',
          }}
          aria-label={t('settings.cancel')}
        >
          <Icon name="x" size={18} />
        </button>
      </div>
      <div
        ref={scrollerRef}
        className="v-scroll"
        style={{
          flex: 1,
          padding: 18,
          display: 'flex',
          flexDirection: 'column',
          gap: 14,
          overflowY: 'auto',
        }}
      >
        {messages.length === 0 && (
          <div
            style={{
              color: 'rgba(255,255,255,0.4)',
              fontSize: 13,
              textAlign: 'center',
              marginTop: 24,
            }}
          >
            {t('chat.noMessages')}
          </div>
        )}
        {messages.map((m) => {
          const me = m.sender_sid === localSid
          return (
            <div
              key={m.id}
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: me ? 'flex-end' : 'flex-start',
                gap: 5,
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
                {!me && <Avatar name={m.sender_name || '—'} size={20} />}
                <span
                  style={{
                    color: 'rgba(255,255,255,0.55)',
                    fontSize: 11.5,
                    fontWeight: 600,
                  }}
                >
                  {me ? t('call.you') : m.sender_name || '—'}
                </span>
              </div>
              <div
                style={{
                  maxWidth: 250,
                  background: me ? 'var(--accent)' : 'rgba(255,255,255,0.08)',
                  color: '#fff',
                  fontSize: 13.5,
                  lineHeight: 1.4,
                  padding: '8px 12px',
                  borderRadius: 14,
                  borderTopLeftRadius: me ? 14 : 4,
                  borderTopRightRadius: me ? 4 : 14,
                  wordBreak: 'break-word',
                }}
              >
                {m.text}
              </div>
            </div>
          )
        })}
      </div>
      <div
        style={{
          padding: 12,
          borderTop: '1px solid rgba(255,255,255,0.08)',
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}
      >
        <div
          style={{
            flex: 1,
            height: 42,
            borderRadius: 12,
            background: 'rgba(255,255,255,0.08)',
            display: 'flex',
            alignItems: 'center',
            padding: '0 12px',
          }}
        >
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                submit()
              }
            }}
            placeholder={t('call.discussion.write')}
            aria-label={t('call.discussion.write')}
            style={{
              flex: 1,
              border: 'none',
              background: 'transparent',
              color: '#fff',
              fontFamily: 'var(--font-ui)',
              fontSize: 13.5,
              outline: 'none',
            }}
          />
        </div>
        <IconBtn
          name="send"
          dim={42}
          size={18}
          variant="accent"
          onClick={submit}
          ariaLabel={t('accessibility.send')}
        />
      </div>
    </div>
  )
}

interface DeskCtrlProps {
  name: string
  label: string
  caret?: boolean
  danger?: boolean
  wide?: boolean
  active?: boolean
  onClick: () => void
  onCaretClick?: () => void
  showsMenu?: boolean
  children?: ReactNode
}
function DeskCtrl({
  name,
  label,
  caret,
  danger,
  wide,
  active,
  onClick,
  onCaretClick,
  showsMenu,
  children,
}: DeskCtrlProps) {
  return (
    <div
      style={{
        position: 'relative',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 6,
      }}
    >
      <div style={{ position: 'relative' }}>
        <button
          onClick={onClick}
          aria-label={label}
          style={{
            height: 48,
            width: wide ? 'auto' : 48,
            padding: wide ? '0 22px' : 0,
            borderRadius: wide ? 14 : '50%',
            border: 'none',
            cursor: 'pointer',
            background: danger
              ? 'var(--danger)'
              : active
                ? 'rgba(255,255,255,0.22)'
                : 'rgba(255,255,255,0.13)',
            color: '#fff',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 9,
            backdropFilter: 'blur(8px)',
            fontFamily: 'var(--font-ui)',
            fontWeight: 600,
            fontSize: 14.5,
            transition: 'background .15s',
          }}
        >
          <Icon name={name} size={20} />
          {wide && label}
        </button>
        {caret && (
          <button
            onClick={onCaretClick}
            aria-label={`${label} — options`}
            style={{
              position: 'absolute',
              top: -2,
              right: -2,
              width: 18,
              height: 18,
              borderRadius: '50%',
              border: 'none',
              cursor: 'pointer',
              background: '#2b2f3a',
              color: '#fff',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              padding: 0,
              boxShadow: '0 0 0 2px rgba(16,18,24,0.95)',
            }}
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 12 12"
              fill="none"
              stroke="#fff"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d={showsMenu ? 'M3 4.5l3 3 3-3' : 'M3 7.5l3-3 3 3'} />
            </svg>
          </button>
        )}
      </div>
      {!wide && (
        <span
          style={{
            fontSize: 11.5,
            color: 'rgba(255,255,255,0.6)',
            fontWeight: 500,
          }}
        >
          {label}
        </span>
      )}
      {children}
    </div>
  )
}

interface AudioPickerProps {
  t: TFunction
  audioInputs: NativeAudioDevice[]
  audioOutputs: NativeAudioDevice[]
  selectedInput: string
  selectedOutput: string
  onPickInput: (name: string) => void
  onPickOutput: (name: string) => void
  onOpenSettings: () => void
  onClose: () => void
}
function AudioPicker({
  t,
  audioInputs,
  audioOutputs,
  selectedInput,
  selectedOutput,
  onPickInput,
  onPickOutput,
  onOpenSettings,
  onClose,
}: AudioPickerProps) {
  return (
    <CallPicker title={t('devicePicker.audio.title')} onClose={onClose}>
      <Section label={t('devicePicker.audio.mic')}>
        {audioInputs.length === 0 ? (
          <Empty label="—" />
        ) : (
          audioInputs.map((d) => (
            <PickerRow
              key={d.name}
              label={d.name}
              active={d.name === selectedInput}
              onClick={() => onPickInput(d.name)}
            />
          ))
        )}
      </Section>
      <Section label={t('devicePicker.audio.speaker')}>
        {audioOutputs.length === 0 ? (
          <Empty label="—" />
        ) : (
          audioOutputs.map((d) => (
            <PickerRow
              key={d.name}
              label={d.name}
              active={d.name === selectedOutput}
              onClick={() => onPickOutput(d.name)}
            />
          ))
        )}
      </Section>
      <Divider />
      <FooterLink
        label={t('devicePicker.audio.openSettings')}
        onClick={onOpenSettings}
      />
    </CallPicker>
  )
}

interface VideoPickerProps {
  t: TFunction
  videoInputs: NativeVideoDevice[]
  selectedInput: string
  bgMode: string
  onPickInput: (uniqueId: string) => void
  onSetBgMode: (mode: string) => void
  onOpenSettings: () => void
  onClose: () => void
}
function VideoPicker({
  t,
  videoInputs,
  selectedInput,
  bgMode,
  onPickInput,
  onSetBgMode,
  onOpenSettings,
  onClose,
}: VideoPickerProps) {
  return (
    <CallPicker title={t('devicePicker.video.title')} onClose={onClose}>
      <Section label={t('devicePicker.video.camera')}>
        {videoInputs.length === 0 ? (
          <Empty label="—" />
        ) : (
          videoInputs.map((d) => (
            <PickerRow
              key={d.unique_id}
              label={d.name}
              active={d.unique_id === selectedInput}
              onClick={() => onPickInput(d.unique_id)}
            />
          ))
        )}
      </Section>
      <Section label={t('devicePicker.video.effects')}>
        <PickerRow
          label={t('settings.row.background.off')}
          active={bgMode === 'off'}
          onClick={() => onSetBgMode('off')}
        />
        <PickerRow
          label={t('devicePicker.video.effects.blur')}
          active={bgMode === 'blur'}
          onClick={() => onSetBgMode('blur')}
        />
      </Section>
      <Divider />
      <FooterLink
        label={t('devicePicker.video.openSettings')}
        onClick={onOpenSettings}
      />
    </CallPicker>
  )
}

interface CallPickerProps {
  title: string
  children: ReactNode
  onClose: () => void
}
function CallPicker({ title, children, onClose }: CallPickerProps) {
  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      const tgt = e.target as Element
      if (
        !tgt.closest('[data-call-picker], [data-call-caret], [data-call-btn]')
      ) {
        onClose()
      }
    }
    document.addEventListener('mousedown', onDoc)
    return () => document.removeEventListener('mousedown', onDoc)
  }, [onClose])
  return (
    <div
      data-call-picker
      style={{
        position: 'absolute',
        bottom: 'calc(100% + 14px)',
        left: '50%',
        transform: 'translateX(-50%)',
        minWidth: 280,
        background: '#1c1f26',
        color: '#fff',
        borderRadius: 14,
        padding: 8,
        boxShadow:
          '0 18px 50px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.06)',
      }}
    >
      <div
        style={{
          padding: '8px 10px 6px',
          fontSize: 11,
          fontWeight: 700,
          letterSpacing: '0.08em',
          textTransform: 'uppercase',
          color: 'rgba(255,255,255,0.5)',
        }}
      >
        {title}
      </div>
      {children}
    </div>
  )
}

interface SectionProps {
  label: string
  children: ReactNode
}
function Section({ label, children }: SectionProps) {
  return (
    <div style={{ marginTop: 4 }}>
      <div
        style={{
          padding: '4px 10px',
          fontSize: 11,
          fontWeight: 700,
          letterSpacing: '0.05em',
          textTransform: 'uppercase',
          color: 'rgba(255,255,255,0.35)',
        }}
      >
        {label}
      </div>
      {children}
    </div>
  )
}

interface PickerRowProps {
  label: string
  active: boolean
  onClick: () => void
}
function PickerRow({ label, active, onClick }: PickerRowProps) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 10,
        width: '100%',
        padding: '8px 10px',
        border: 'none',
        background: 'transparent',
        color: '#fff',
        fontFamily: 'var(--font-ui)',
        fontSize: 13.5,
        cursor: 'pointer',
        borderRadius: 8,
        textAlign: 'left',
      }}
      onMouseEnter={(e) =>
        (e.currentTarget.style.background = 'rgba(255,255,255,0.06)')
      }
      onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
    >
      <Icon
        name="check"
        size={14}
        style={{ color: active ? 'var(--accent)' : 'transparent' }}
      />
      <span
        style={{
          flex: 1,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}
      >
        {label}
      </span>
    </button>
  )
}

function Empty({ label }: { label: string }) {
  return (
    <div
      style={{
        padding: '10px 12px',
        fontSize: 13,
        color: 'rgba(255,255,255,0.4)',
      }}
    >
      {label}
    </div>
  )
}
function Divider() {
  return (
    <hr
      style={{
        border: 'none',
        borderTop: '1px solid rgba(255,255,255,0.08)',
        margin: '6px 4px',
      }}
    />
  )
}
function FooterLink({
  label,
  onClick,
}: {
  label: string
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'block',
        width: '100%',
        padding: '8px 10px',
        border: 'none',
        background: 'transparent',
        color: 'var(--accent)',
        fontFamily: 'var(--font-ui)',
        fontSize: 13,
        fontWeight: 600,
        cursor: 'pointer',
        borderRadius: 8,
        textAlign: 'left',
      }}
    >
      {label}
    </button>
  )
}

// ---------------------------------------------------------------------------
// CallScreen — main composition
// ---------------------------------------------------------------------------

export function CallScreen(props: CallScreenProps) {
  const {
    t,
    roomTitle,
    participants,
    localParticipant,
    micEnabled,
    camEnabled,
    isHandRaised,
    videoFrames,
    activeSpeakers,
    handRaisedMap,
    messages,
    encrypted,
    onSendChat,
    onToggleMic,
    onToggleCam,
    onHangUp,
    onToggleHandRaise,
    onToggleScreenShare,
    onReact,
    audioInputs,
    audioOutputs,
    videoInputs,
    selectedAudioInput,
    selectedAudioOutput,
    selectedVideoInput,
    onSelectAudioInput,
    onSelectAudioOutput,
    onSelectVideoInput,
    onShowMicPicker,
    onShowCamPicker,
    showMicPicker,
    showCamPicker,
    onClosePickers,
    onOpenSettings,
    bgMode,
    onSetBgMode,
    callStartedMs,
    layoutMode,
    onToggleLayout,
    onTogglePeople,
    peopleOpen,
  } = props

  const [chatOpen, setChatOpenState] = useState(true)
  const [reactionOpen, setReactionOpen] = useState(false)

  // Keep Rust's chat_open state in sync so the unread badge stays consistent
  // (Rust only increments unread when the panel is hidden).
  const setChatOpen = (next: boolean) => {
    setChatOpenState(next)
    invoke('set_chat_open', { open: next }).catch(() => {})
  }
  useEffect(() => {
    invoke('set_chat_open', { open: true }).catch(() => {})
    return () => {
      invoke('set_chat_open', { open: false }).catch(() => {})
    }
  }, [])

  // Timer ticking once per second, anchored to the real connect timestamp
  // owned by App.tsx (Date.now() at the rising edge of 'connected').
  const [now, setNow] = useState(Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(id)
  }, [])
  const timer = fmtElapsed(callStartedMs ?? now, now)

  const items = useMemo(
    () =>
      buildDisplayItems(
        localParticipant,
        participants,
        activeSpeakers,
        handRaisedMap,
        t
      ),
    [localParticipant, participants, activeSpeakers, handRaisedMap, t]
  )

  const cols = gridCols(items.length)
  const rows = gridRows(items.length)
  const total = items.length

  return (
    <div
      style={{
        width: '100%',
        height: '100vh',
        background: '#0a0b0e',
        color: '#fff',
        position: 'relative',
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {/* background tint */}
      <div
        aria-hidden
        style={{
          position: 'absolute',
          inset: 0,
          background:
            'radial-gradient(90% 50% at 40% 0%, color-mix(in oklab, var(--accent) 24%, transparent), transparent 55%)',
          pointerEvents: 'none',
        }}
      />

      {/* top bar */}
      <div
        style={{
          position: 'relative',
          zIndex: 5,
          height: 56,
          flexShrink: 0,
          display: 'flex',
          alignItems: 'center',
          gap: 16,
          padding: '0 20px',
        }}
      >
        <VisioMark size={22} dark />
        <div>
          <div
            style={{
              color: '#fff',
              fontSize: 14.5,
              fontWeight: 700,
              letterSpacing: '-0.02em',
            }}
          >
            {roomTitle || t('call.title')}
          </div>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 7,
              whiteSpace: 'nowrap',
            }}
          >
            <span
              style={{
                width: 7,
                height: 7,
                borderRadius: '50%',
                background: 'var(--live)',
              }}
            />
            <span
              className="v-mono"
              style={{
                color: 'rgba(255,255,255,0.7)',
                fontSize: 12.5,
              }}
            >
              {timer}
            </span>
            <span
              style={{
                color: 'rgba(255,255,255,0.45)',
                fontSize: 12.5,
              }}
            >
              · {total} participants
            </span>
          </div>
        </div>
        <div style={{ flex: 1 }} />
        {encrypted && (
          <span
            className="v-chip"
            style={{
              height: 32,
              background: 'rgba(255,255,255,0.1)',
              color: '#fff',
            }}
          >
            <Icon name="shield" size={14} /> {t('call.encrypted')}
          </span>
        )}
        <IconBtn
          name={layoutMode === 'speaker' ? 'pin' : 'grid'}
          dim={36}
          size={18}
          variant="glass"
          tint="#fff"
          onClick={onToggleLayout}
          ariaLabel={
            layoutMode === 'speaker'
              ? t('layout.switchToGrid')
              : t('layout.switchToSpeaker')
          }
        />
      </div>

      {/* body: video grid + chat panel */}
      <div
        style={{
          position: 'relative',
          zIndex: 4,
          flex: 1,
          minHeight: 0,
          display: 'flex',
        }}
      >
        <div
          style={{
            flex: 1,
            minWidth: 0,
            padding: '4px 16px 110px 20px',
          }}
        >
          {total === 0 ? (
            <div
              style={{
                width: '100%',
                height: '100%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'rgba(255,255,255,0.5)',
                fontSize: 14,
              }}
            >
              {t('call.waiting')}
            </div>
          ) : (
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: cols,
                gridTemplateRows: rows,
                gap: 12,
                height: '100%',
              }}
            >
              {items.map((it) => (
                <CallTile
                  key={it.key}
                  item={it}
                  frame={
                    it.trackSid ? (videoFrames.get(it.trackSid) ?? null) : null
                  }
                  big={total === 1}
                />
              ))}
            </div>
          )}
        </div>
        {chatOpen && (
          <ChatPanel
            t={t}
            messages={messages}
            localSid={localParticipant?.sid ?? null}
            onSend={onSendChat}
            onClose={() => setChatOpen(false)}
          />
        )}
      </div>

      {/* bottom control bar */}
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: chatOpen ? 340 : 0,
          bottom: 18,
          zIndex: 6,
          display: 'flex',
          alignItems: 'flex-start',
          justifyContent: 'center',
          gap: 14,
          pointerEvents: 'none',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'flex-start',
            gap: 14,
            pointerEvents: 'auto',
          }}
        >
          <div style={{ position: 'relative' }} data-call-btn>
            <DeskCtrl
              name={micEnabled ? 'mic' : 'micOff'}
              label={t('call.label.mic')}
              caret
              active={micEnabled}
              showsMenu={showMicPicker}
              onClick={onToggleMic}
              onCaretClick={onShowMicPicker}
            />
            {showMicPicker && (
              <AudioPicker
                t={t}
                audioInputs={audioInputs}
                audioOutputs={audioOutputs}
                selectedInput={selectedAudioInput}
                selectedOutput={selectedAudioOutput}
                onPickInput={(n) => {
                  onSelectAudioInput(n)
                  onClosePickers()
                }}
                onPickOutput={(n) => {
                  onSelectAudioOutput(n)
                  onClosePickers()
                }}
                onOpenSettings={() => {
                  onClosePickers()
                  onOpenSettings()
                }}
                onClose={onClosePickers}
              />
            )}
          </div>
          <div style={{ position: 'relative' }} data-call-btn>
            <DeskCtrl
              name={camEnabled ? 'video' : 'videoOff'}
              label={t('call.label.camera')}
              caret
              active={camEnabled}
              showsMenu={showCamPicker}
              onClick={onToggleCam}
              onCaretClick={onShowCamPicker}
            />
            {showCamPicker && (
              <VideoPicker
                t={t}
                videoInputs={videoInputs}
                selectedInput={selectedVideoInput}
                bgMode={bgMode}
                onPickInput={(id) => {
                  onSelectVideoInput(id)
                  onClosePickers()
                }}
                onSetBgMode={(m) => {
                  onSetBgMode(m)
                  onClosePickers()
                }}
                onOpenSettings={() => {
                  onClosePickers()
                  onOpenSettings()
                }}
                onClose={onClosePickers}
              />
            )}
          </div>
          <DeskCtrl
            name="screenShare"
            label={t('call.label.share')}
            onClick={onToggleScreenShare}
          />
          <DeskCtrl
            name="sparkle"
            label={t('call.label.effects')}
            onClick={() => onShowCamPicker()}
          />
          <div style={{ position: 'relative' }} data-call-btn>
            <DeskCtrl
              name="smiley"
              label={t('call.label.react')}
              active={reactionOpen}
              onClick={() => setReactionOpen((v) => !v)}
            />
            {reactionOpen && (
              <ReactionPicker
                onPick={(e) => {
                  onReact(e)
                  setReactionOpen(false)
                }}
                onClose={() => setReactionOpen(false)}
              />
            )}
          </div>
          <DeskCtrl
            name="hand"
            label={
              isHandRaised ? t('control.lowerHand') : t('control.raiseHand')
            }
            active={isHandRaised}
            onClick={onToggleHandRaise}
          />
          <DeskCtrl
            name="users"
            label={t('call.label.people')}
            active={peopleOpen}
            onClick={onTogglePeople}
          />
          <div style={{ width: 10 }} />
          <DeskCtrl
            name="hangup"
            label={t('call.leave')}
            danger
            wide
            onClick={onHangUp}
          />
        </div>
      </div>
    </div>
  )
}

interface ReactionPickerProps {
  onPick: (emoji: string) => void
  onClose: () => void
}
function ReactionPicker({ onPick, onClose }: ReactionPickerProps) {
  useEffect(() => {
    const off = (e: MouseEvent) => {
      const tgt = e.target as Element
      if (!tgt.closest('[data-call-picker], [data-call-btn]')) onClose()
    }
    document.addEventListener('mousedown', off)
    return () => document.removeEventListener('mousedown', off)
  }, [onClose])
  const emojis = ['👍', '👏', '😂', '😮', '🎉', '❤️']
  return (
    <div
      data-call-picker
      style={{
        position: 'absolute',
        bottom: 'calc(100% + 14px)',
        left: '50%',
        transform: 'translateX(-50%)',
        background: '#1c1f26',
        borderRadius: 14,
        padding: 8,
        display: 'flex',
        gap: 4,
        boxShadow: '0 18px 50px rgba(0,0,0,0.55)',
      }}
    >
      {emojis.map((e) => (
        <button
          key={e}
          onClick={() => onPick(e)}
          style={{
            width: 38,
            height: 38,
            border: 'none',
            background: 'transparent',
            cursor: 'pointer',
            fontSize: 20,
            borderRadius: 10,
          }}
          onMouseEnter={(ev) =>
            (ev.currentTarget.style.background = 'rgba(255,255,255,0.08)')
          }
          onMouseLeave={(ev) =>
            (ev.currentTarget.style.background = 'transparent')
          }
        >
          {e}
        </button>
      ))}
    </div>
  )
}

export default CallScreen
