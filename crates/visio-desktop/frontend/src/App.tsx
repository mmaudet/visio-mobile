import { useState, useEffect, useRef, useCallback, createContext, useContext } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  RiMicLine,
  RiMicOffLine,
  RiMicOffFill,
  RiVideoOnLine,
  RiVideoOffLine,
  RiArrowUpSLine,
  RiHand,
  RiChat1Line,
  RiPhoneFill,
  RiCloseLine,
  RiSendPlane2Fill,
  RiSettings3Line,
} from "@remixicon/react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type View = "home" | "call";

interface Participant {
  sid: string;
  identity: string;
  name: string | null;
  is_muted: boolean;
  has_video: boolean;
  video_track_sid: string | null;
  connection_quality: string;
}

interface ChatMessage {
  id: string;
  sender_sid: string;
  sender_name: string | null;
  text: string;
  timestamp_ms: number;
}

interface VideoFrame {
  track_sid: string;
  data: string; // base64 JPEG
  width: number;
  height: number;
}

interface Settings {
  display_name: string | null;
  language: string | null;
  mic_enabled_on_join: boolean;
  camera_enabled_on_join: boolean;
  theme: string;
}

// ---------------------------------------------------------------------------
// i18n
// ---------------------------------------------------------------------------

type TFunction = (key: string) => string;
const I18nContext = createContext<TFunction>((key) => key);
function useT() {
  return useContext(I18nContext);
}

const translations: Record<string, Record<string, string>> = {
  en: {
    "home.subtitle": "Enter a meeting URL and your display name",
    "home.meetUrl": "Meeting URL",
    "home.meetUrl.placeholder": "meet.example.com/my-room",
    "home.displayName": "Display Name (optional)",
    "home.displayName.placeholder": "Your name",
    "home.join": "Join",
    "home.connecting": "Connecting...",
    "home.error.noUrl": "Please enter a meeting URL",
    "call.noParticipants": "No other participants yet",
    "call.you": "You",
    "chat": "Chat",
    "chat.noMessages": "No messages yet",
    "chat.placeholder": "Message",
    "unknown": "Unknown",
    "control.mute": "Mute microphone",
    "control.unmute": "Unmute microphone",
    "control.audioDevices": "Audio devices",
    "control.camOff": "Turn off camera",
    "control.camOn": "Turn on camera",
    "control.camDevices": "Camera devices",
    "control.lowerHand": "Lower hand",
    "control.raiseHand": "Raise hand",
    "control.leave": "Leave call",
    "device.microphone": "Microphone",
    "device.speaker": "Speaker",
    "device.camera": "Camera",
    "device.noMic": "No microphones found",
    "device.noSpeaker": "No speakers found",
    "device.noCamera": "No cameras found",
    "settings": "Settings",
    "settings.displayName": "Display name",
    "settings.language": "Language",
    "settings.micOnJoin": "Mic on join",
    "settings.camOnJoin": "Camera on join",
    "settings.theme": "Theme",
    "settings.theme.light": "Light",
    "settings.theme.dark": "Dark",
    "settings.save": "Save",
    "status.disconnected": "disconnected",
    "status.connected": "connected",
    "status.connecting": "connecting",
    "status.reconnecting": "reconnecting",
  },
  fr: {
    "home.subtitle": "Entrez l'URL de la réunion et votre nom",
    "home.meetUrl": "URL de la réunion",
    "home.meetUrl.placeholder": "meet.example.com/ma-reunion",
    "home.displayName": "Nom d'affichage (optionnel)",
    "home.displayName.placeholder": "Votre nom",
    "home.join": "Rejoindre",
    "home.connecting": "Connexion...",
    "home.error.noUrl": "Veuillez entrer une URL de réunion",
    "call.noParticipants": "Aucun autre participant pour le moment",
    "call.you": "Vous",
    "chat": "Discussion",
    "chat.noMessages": "Aucun message pour le moment",
    "chat.placeholder": "Message",
    "unknown": "Inconnu",
    "control.mute": "Couper le micro",
    "control.unmute": "Activer le micro",
    "control.audioDevices": "Périphériques audio",
    "control.camOff": "Désactiver la caméra",
    "control.camOn": "Activer la caméra",
    "control.camDevices": "Caméras",
    "control.lowerHand": "Baisser la main",
    "control.raiseHand": "Lever la main",
    "control.leave": "Quitter l'appel",
    "device.microphone": "Microphone",
    "device.speaker": "Haut-parleur",
    "device.camera": "Caméra",
    "device.noMic": "Aucun micro trouvé",
    "device.noSpeaker": "Aucun haut-parleur trouvé",
    "device.noCamera": "Aucune caméra trouvée",
    "settings": "Paramètres",
    "settings.displayName": "Nom d'affichage",
    "settings.language": "Langue",
    "settings.micOnJoin": "Micro activé en rejoignant",
    "settings.camOnJoin": "Caméra activée en rejoignant",
    "settings.theme": "Thème",
    "settings.theme.light": "Clair",
    "settings.theme.dark": "Sombre",
    "settings.save": "Enregistrer",
    "status.disconnected": "déconnecté",
    "status.connected": "connecté",
    "status.connecting": "connexion",
    "status.reconnecting": "reconnexion",
  },
};

// ---------------------------------------------------------------------------
// Logo SVG tricolore
// ---------------------------------------------------------------------------

function VisioLogo({ size = 64 }: { size?: number }) {
  // Camera body: 64×54 (ratio ~1.19), centered at x=52
  // Wifi arcs: 3 concentric arcs (r=10,17,24) centered at (52,62), pointing up
  // Stripe: same width as camera body (64), centered on same axis
  const stripeX = 20;
  const thirdW = 64 / 3;
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
      <path d="M45 55 A10 10 0 0 1 59 55" stroke="#fff" strokeWidth="3" strokeLinecap="round" fill="none" />
      {/* Wifi arc — medium (r=17) */}
      <path d="M40 50 A17 17 0 0 1 64 50" stroke="#fff" strokeWidth="3" strokeLinecap="round" fill="none" />
      {/* Wifi arc — large (r=24) */}
      <path d="M35 45 A24 24 0 0 1 69 45" stroke="#fff" strokeWidth="3" strokeLinecap="round" fill="none" />
      {/* Tricolore stripe — centered under camera body */}
      <rect x={stripeX} y="92" width={thirdW} height="9" rx="3" fill="#000091" />
      <rect x={stripeX + thirdW} y="92" width={thirdW} height="9" fill="#FFFFFF" stroke="#D1D1D6" strokeWidth="0.5" />
      <rect x={stripeX + thirdW * 2} y="92" width={thirdW} height="9" rx="3" fill="#E1000F" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function getInitials(name: string | null | undefined): string {
  if (!name) return "?";
  const parts = name.trim().split(/\s+/);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return name.substring(0, 2).toUpperCase();
}

function getHue(name: string | null | undefined): number {
  return [...(name || "")].reduce((h, c) => h + c.charCodeAt(0), 0) % 360;
}

function formatTime(timestampMs: number): string {
  if (!timestampMs) return "";
  const d = new Date(timestampMs);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

function StatusBadge({ state }: { state: string }) {
  const t = useT();
  const key = `status.${state}`;
  return <span className={`status-badge ${state}`}>{t(key)}</span>;
}

// -- Connection Quality Bars ------------------------------------------------

function ConnectionQualityBars({ quality }: { quality: string }) {
  const bars =
    quality === "Excellent" ? 3 : quality === "Good" ? 2 : quality === "Poor" ? 1 : 0;
  return (
    <div className="connection-bars">
      {[1, 2, 3].map((i) => (
        <div
          key={i}
          className={`bar ${i <= bars ? "bar-active" : ""}`}
          style={{ height: `${i * 4 + 2}px` }}
        />
      ))}
    </div>
  );
}

// -- Participant Tile -------------------------------------------------------

interface ParticipantTileProps {
  participant: Participant;
  videoFrames: Map<string, string>;
  isActiveSpeaker?: boolean;
  handRaisePosition?: number;
}

function ParticipantTile({
  participant,
  videoFrames,
  isActiveSpeaker,
  handRaisePosition,
}: ParticipantTileProps) {
  const t = useT();
  const displayName = participant.name || participant.identity || t("unknown");
  const initials = getInitials(displayName);
  const hue = getHue(displayName);

  const videoSrc = participant.video_track_sid
    ? videoFrames.get(participant.video_track_sid)
    : undefined;

  return (
    <div className={`tile ${isActiveSpeaker ? "tile-active-speaker" : ""}`}>
      {videoSrc ? (
        <img
          className="tile-video"
          src={`data:image/jpeg;base64,${videoSrc}`}
          alt=""
        />
      ) : (
        <div
          className="tile-avatar"
          style={{ background: `hsl(${hue}, 50%, 35%)` }}
        >
          <span className="tile-initials">{initials}</span>
        </div>
      )}
      <div className="tile-metadata">
        {participant.is_muted && (
          <span className="tile-muted-icon">
            <RiMicOffFill size={14} />
          </span>
        )}
        {handRaisePosition != null && handRaisePosition > 0 && (
          <span className="tile-hand-badge">
            <RiHand size={12} /> {handRaisePosition}
          </span>
        )}
        <span className="tile-name">{displayName}</span>
        <ConnectionQualityBars quality={participant.connection_quality} />
      </div>
    </div>
  );
}

// -- Home View --------------------------------------------------------------

function HomeView({
  onJoin,
  onOpenSettings,
  displayName,
  onDisplayNameChange,
}: {
  onJoin: (meetUrl: string, username: string | null) => void;
  onOpenSettings: () => void;
  displayName: string;
  onDisplayNameChange: (name: string) => void;
}) {
  const t = useT();
  const [meetUrl, setMeetUrl] = useState("");
  const [error, setError] = useState("");
  const [joining, setJoining] = useState(false);

  const handleJoin = async () => {
    const url = meetUrl.trim();
    if (!url) {
      setError(t("home.error.noUrl"));
      return;
    }
    setError("");
    setJoining(true);
    try {
      const uname = displayName.trim() || null;
      await invoke("set_display_name", { name: uname });
      await invoke("connect", { meetUrl: url, username: uname });
      onJoin(url, uname);
    } catch (e) {
      setError(String(e));
      setJoining(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleJoin();
  };

  return (
    <div id="home" className="section active">
      <button className="settings-gear" onClick={onOpenSettings}>
        <RiSettings3Line size={24} />
      </button>
      <div className="join-form">
        <VisioLogo />
        <h2>Visio Mobile</h2>
        <p>{t("home.subtitle")}</p>
        <div className="form-group">
          <label htmlFor="meetUrl">{t("home.meetUrl")}</label>
          <input
            id="meetUrl"
            type="text"
            placeholder={t("home.meetUrl.placeholder")}
            autoComplete="off"
            value={meetUrl}
            onChange={(e) => setMeetUrl(e.target.value)}
            onKeyDown={handleKeyDown}
          />
        </div>
        <div className="form-group">
          <label htmlFor="username">{t("home.displayName")}</label>
          <input
            id="username"
            type="text"
            placeholder={t("home.displayName.placeholder")}
            autoComplete="off"
            value={displayName}
            onChange={(e) => onDisplayNameChange(e.target.value)}
            onKeyDown={handleKeyDown}
          />
        </div>
        <button className="btn btn-primary" disabled={joining} onClick={handleJoin}>
          {joining ? t("home.connecting") : t("home.join")}
        </button>
        <div className="error-msg">{error}</div>
      </div>
    </div>
  );
}

// -- Call View --------------------------------------------------------------

function CallView({
  participants,
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
  onShowMicPicker,
  onShowCamPicker,
  showMicPicker,
  showCamPicker,
  audioInputs,
  audioOutputs,
  videoInputs,
  selectedAudioInput,
  selectedVideoInput,
  onSelectAudioInput,
  onSelectVideoInput,
}: {
  participants: Participant[];
  micEnabled: boolean;
  camEnabled: boolean;
  videoFrames: Map<string, string>;
  messages: ChatMessage[];
  handRaisedMap: Record<string, number>;
  isHandRaised: boolean;
  unreadCount: number;
  showChat: boolean;
  onToggleMic: () => void;
  onToggleCam: () => void;
  onHangUp: () => void;
  onToggleHandRaise: () => void;
  onToggleChat: () => void;
  onSendChat: (text: string) => void;
  onShowMicPicker: () => void;
  onShowCamPicker: () => void;
  showMicPicker: boolean;
  showCamPicker: boolean;
  audioInputs: MediaDeviceInfo[];
  audioOutputs: MediaDeviceInfo[];
  videoInputs: MediaDeviceInfo[];
  selectedAudioInput: string;
  selectedVideoInput: string;
  onSelectAudioInput: (id: string) => void;
  onSelectVideoInput: (id: string) => void;
}) {
  const t = useT();
  const [focusedParticipant, setFocusedParticipant] = useState<string | null>(null);
  const [chatInput, setChatInput] = useState("");
  const chatScrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (chatScrollRef.current) {
      chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight;
    }
  }, [messages.length]);

  const sendMessage = () => {
    const text = chatInput.trim();
    if (!text) return;
    setChatInput("");
    onSendChat(text);
  };

  const localFrame = videoFrames.get("local-camera");
  const gridCount = Math.min(participants.length, 9);

  return (
    <div id="call" className="section active">
      {/* Self-view PiP */}
      {camEnabled && localFrame && (
        <div className={`self-view ${showChat ? "with-chat" : ""}`}>
          <img
            className="self-video"
            src={`data:image/jpeg;base64,${localFrame}`}
            alt="self-view"
          />
          <span className="self-label">{t("call.you")}</span>
        </div>
      )}

      {/* Main video area */}
      <div className={`call-content ${showChat ? "with-chat" : ""}`}>
        {focusedParticipant && participants.find((p) => p.sid === focusedParticipant) ? (
          <div className="focus-layout">
            <div className="focus-main" onClick={() => setFocusedParticipant(null)}>
              <ParticipantTile
                participant={participants.find((p) => p.sid === focusedParticipant)!}
                videoFrames={videoFrames}
                handRaisePosition={handRaisedMap[focusedParticipant]}
              />
            </div>
            <div className="focus-strip">
              {participants
                .filter((p) => p.sid !== focusedParticipant)
                .map((p) => (
                  <div key={p.sid} onClick={() => setFocusedParticipant(p.sid)}>
                    <ParticipantTile
                      participant={p}
                      videoFrames={videoFrames}
                      handRaisePosition={handRaisedMap[p.sid]}
                    />
                  </div>
                ))}
            </div>
          </div>
        ) : (
          <div className={`video-grid video-grid-${gridCount}`}>
            {participants.length === 0 ? (
              <div className="empty-state">{t("call.noParticipants")}</div>
            ) : (
              participants.map((p) => (
                <div key={p.sid} onClick={() => setFocusedParticipant(p.sid)}>
                  <ParticipantTile
                    participant={p}
                    videoFrames={videoFrames}
                    handRaisePosition={handRaisedMap[p.sid]}
                  />
                </div>
              ))
            )}
          </div>
        )}
      </div>

      {/* Chat sidebar */}
      {showChat && (
        <div className="chat-sidebar">
          <div className="chat-header">
            <span>{t("chat")}</span>
            <button className="chat-close" onClick={onToggleChat}>
              <RiCloseLine size={20} />
            </button>
          </div>
          <div className="chat-messages" ref={chatScrollRef}>
            {messages.length === 0 ? (
              <div className="chat-empty">{t("chat.noMessages")}</div>
            ) : (
              messages.map((m, i) => {
                const showName =
                  i === 0 || messages[i - 1].sender_sid !== m.sender_sid;
                return (
                  <div key={m.id} className="chat-bubble">
                    {showName && (
                      <div className="chat-sender">
                        {m.sender_name || t("unknown")}
                      </div>
                    )}
                    <div className="chat-text">{m.text}</div>
                    <div className="chat-time">{formatTime(m.timestamp_ms)}</div>
                  </div>
                );
              })
            )}
          </div>
          <div className="chat-input-bar">
            <input
              className="chat-input"
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && sendMessage()}
              placeholder={t("chat.placeholder")}
            />
            <button
              className="chat-send"
              onClick={sendMessage}
              disabled={!chatInput.trim()}
            >
              <RiSendPlane2Fill size={18} />
            </button>
          </div>
        </div>
      )}

      {/* Control bar */}
      <div className="control-bar">
        {/* Mic group */}
        <div className="control-group">
          <button
            className={`control-btn ${micEnabled ? "" : "control-btn-off"}`}
            onClick={onToggleMic}
            title={micEnabled ? t("control.mute") : t("control.unmute")}
            style={{ borderRadius: "8px 0 0 8px" }}
          >
            {micEnabled ? <RiMicLine size={20} /> : <RiMicOffLine size={20} />}
          </button>
          <button
            className={`control-btn control-chevron ${micEnabled ? "" : "control-btn-off"}`}
            onClick={onShowMicPicker}
            title={t("control.audioDevices")}
            style={{ borderRadius: "0 8px 8px 0" }}
          >
            <RiArrowUpSLine size={16} />
          </button>
        </div>

        {/* Camera group */}
        <div className="control-group">
          <button
            className={`control-btn ${camEnabled ? "" : "control-btn-off"}`}
            onClick={onToggleCam}
            title={camEnabled ? t("control.camOff") : t("control.camOn")}
            style={{ borderRadius: "8px 0 0 8px" }}
          >
            {camEnabled ? (
              <RiVideoOnLine size={20} />
            ) : (
              <RiVideoOffLine size={20} />
            )}
          </button>
          <button
            className={`control-btn control-chevron ${camEnabled ? "" : "control-btn-off"}`}
            onClick={onShowCamPicker}
            title={t("control.camDevices")}
            style={{ borderRadius: "0 8px 8px 0" }}
          >
            <RiArrowUpSLine size={16} />
          </button>
        </div>

        {/* Hand raise */}
        <button
          className={`control-btn ${isHandRaised ? "control-btn-hand" : ""}`}
          onClick={onToggleHandRaise}
          title={isHandRaised ? t("control.lowerHand") : t("control.raiseHand")}
        >
          <RiHand size={20} />
        </button>

        {/* Chat */}
        <button
          className={`control-btn ${showChat ? "control-btn-hand" : ""}`}
          onClick={onToggleChat}
          title={t("chat")}
        >
          <RiChat1Line size={20} />
          {unreadCount > 0 && (
            <span className="unread-badge">
              {unreadCount > 9 ? "9+" : unreadCount}
            </span>
          )}
        </button>

        {/* Hangup */}
        <button
          className="control-btn control-btn-hangup"
          onClick={onHangUp}
          title={t("control.leave")}
        >
          <RiPhoneFill size={20} />
        </button>
      </div>

      {/* Mic device picker */}
      {showMicPicker && (
        <div className="device-picker">
          <div className="device-section">
            <div className="device-section-title">{t("device.microphone")}</div>
            {audioInputs.map((d) => (
              <label key={d.deviceId} className="device-option">
                <input
                  type="radio"
                  name="audioInput"
                  checked={selectedAudioInput === d.deviceId}
                  onChange={() => onSelectAudioInput(d.deviceId)}
                />
                {d.label || t("device.microphone")}
              </label>
            ))}
            {audioInputs.length === 0 && (
              <div style={{ fontSize: "0.8rem", color: "#929292", padding: "4px 8px" }}>
                {t("device.noMic")}
              </div>
            )}
          </div>
          <div className="device-section">
            <div className="device-section-title">{t("device.speaker")}</div>
            {audioOutputs.map((d) => (
              <label key={d.deviceId} className="device-option">
                <input type="radio" name="audioOutput" />
                {d.label || t("device.speaker")}
              </label>
            ))}
            {audioOutputs.length === 0 && (
              <div style={{ fontSize: "0.8rem", color: "#929292", padding: "4px 8px" }}>
                {t("device.noSpeaker")}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Camera device picker */}
      {showCamPicker && (
        <div className="device-picker">
          <div className="device-section">
            <div className="device-section-title">{t("device.camera")}</div>
            {videoInputs.map((d) => (
              <label key={d.deviceId} className="device-option">
                <input
                  type="radio"
                  name="videoInput"
                  checked={selectedVideoInput === d.deviceId}
                  onChange={() => onSelectVideoInput(d.deviceId)}
                />
                {d.label || t("device.camera")}
              </label>
            ))}
            {videoInputs.length === 0 && (
              <div style={{ fontSize: "0.8rem", color: "#929292", padding: "4px 8px" }}>
                {t("device.noCamera")}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// -- Settings Modal ---------------------------------------------------------

function SettingsModal({
  onClose,
  onLanguageChange,
  onThemeChange,
  onDisplayNameChange,
  initialDisplayName,
}: {
  onClose: () => void;
  onLanguageChange: (lang: string) => void;
  onThemeChange: (theme: string) => void;
  onDisplayNameChange: (name: string) => void;
  initialDisplayName: string;
}) {
  const t = useT();
  const [form, setForm] = useState({
    displayName: initialDisplayName,
    language: "fr",
    micOnJoin: true,
    cameraOnJoin: false,
    theme: "light",
  });

  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => {
        setForm((prev) => ({
          ...prev,
          language: s.language || "fr",
          micOnJoin: s.mic_enabled_on_join ?? true,
          cameraOnJoin: s.camera_enabled_on_join ?? false,
          theme: s.theme || "light",
        }));
      })
      .catch(() => {});
  }, []);

  const save = async () => {
    await invoke("set_display_name", { name: form.displayName || null });
    await invoke("set_mic_enabled_on_join", { enabled: form.micOnJoin });
    await invoke("set_camera_enabled_on_join", { enabled: form.cameraOnJoin });
    onDisplayNameChange(form.displayName);
    onClose();
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <span>{t("settings")}</span>
          <button onClick={onClose}>
            <RiCloseLine size={20} />
          </button>
        </div>
        <div className="settings-body">
          <div className="settings-section">
            <label className="settings-label">{t("settings.displayName")}</label>
            <input
              className="settings-input"
              value={form.displayName}
              onChange={(e) =>
                setForm({ ...form, displayName: e.target.value })
              }
            />
          </div>
          <div className="settings-section">
            <label className="settings-label">{t("settings.language")}</label>
            <select
              value={form.language}
              onChange={(e) => {
                const lang = e.target.value;
                setForm({ ...form, language: lang });
                invoke("set_language", { lang: lang || null });
                onLanguageChange(lang);
              }}
            >
              <option value="fr">Français</option>
              <option value="en">English</option>
            </select>
          </div>
          <div className="settings-section">
            <label className="settings-label">{t("settings.theme")}</label>
            <select
              value={form.theme}
              onChange={(e) => {
                const theme = e.target.value;
                setForm({ ...form, theme });
                invoke("set_theme", { theme });
                onThemeChange(theme);
              }}
            >
              <option value="light">{t("settings.theme.light")}</option>
              <option value="dark">{t("settings.theme.dark")}</option>
            </select>
          </div>
          <div className="settings-section">
            <label className="settings-label">{t("settings.micOnJoin")}</label>
            <input
              type="checkbox"
              checked={form.micOnJoin}
              onChange={(e) =>
                setForm({ ...form, micOnJoin: e.target.checked })
              }
            />
          </div>
          <div className="settings-section">
            <label className="settings-label">{t("settings.camOnJoin")}</label>
            <input
              type="checkbox"
              checked={form.cameraOnJoin}
              onChange={(e) =>
                setForm({ ...form, cameraOnJoin: e.target.checked })
              }
            />
          </div>
        </div>
        <button className="settings-save" onClick={save}>
          {t("settings.save")}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// App (root)
// ---------------------------------------------------------------------------

export default function App() {
  const [view, setView] = useState<View>("home");
  const [connectionState, setConnectionState] = useState("disconnected");
  const [participants, setParticipants] = useState<Participant[]>([]);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [micEnabled, setMicEnabled] = useState(false);
  const [camEnabled, setCamEnabled] = useState(false);
  const [videoFrames, setVideoFrames] = useState<Map<string, string>>(
    () => new Map()
  );

  // New state for UX overhaul
  const [isHandRaised, setIsHandRaised] = useState(false);
  const [unreadCount, setUnreadCount] = useState(0);
  const [handRaisedMap, setHandRaisedMap] = useState<Record<string, number>>({});
  const [showChat, setShowChat] = useState(false);
  const [showMicPicker, setShowMicPicker] = useState(false);
  const [showCamPicker, setShowCamPicker] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  // Display name (shared between Home and Settings)
  const [displayName, setDisplayName] = useState("");
  // i18n
  const [lang, setLang] = useState("fr");
  // Theme
  const [theme, setTheme] = useState("light");

  const t = useCallback(
    (key: string) => translations[lang]?.[key] ?? translations.en[key] ?? key,
    [lang]
  );

  // Load settings on mount
  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => {
        if (s.display_name) setDisplayName(s.display_name);
        if (s.language) setLang(s.language);
        if (s.theme) setTheme(s.theme);
      })
      .catch(() => {});
  }, []);

  // Apply theme to document
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  // Device enumeration
  const [audioInputs, setAudioInputs] = useState<MediaDeviceInfo[]>([]);
  const [audioOutputs, setAudioOutputs] = useState<MediaDeviceInfo[]>([]);
  const [videoInputs, setVideoInputs] = useState<MediaDeviceInfo[]>([]);
  const [selectedAudioInput, setSelectedAudioInput] = useState("");
  const [selectedVideoInput, setSelectedVideoInput] = useState("");

  const viewRef = useRef(view);
  viewRef.current = view;

  // ---- Device enumeration -------------------------------------------------
  useEffect(() => {
    const enumerate = async () => {
      try {
        const devices = await navigator.mediaDevices.enumerateDevices();
        setAudioInputs(devices.filter((d) => d.kind === "audioinput"));
        setAudioOutputs(devices.filter((d) => d.kind === "audiooutput"));
        setVideoInputs(devices.filter((d) => d.kind === "videoinput"));
      } catch {
        // Not available in Tauri webview without permissions
      }
    };
    enumerate();
    navigator.mediaDevices?.addEventListener("devicechange", enumerate);
    return () => {
      navigator.mediaDevices?.removeEventListener("devicechange", enumerate);
    };
  }, []);

  // ---- Click outside to close device pickers ------------------------------
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (
        !(e.target as Element).closest(".device-picker, .control-chevron")
      ) {
        setShowMicPicker(false);
        setShowCamPicker(false);
      }
    };
    document.addEventListener("click", handleClick);
    return () => document.removeEventListener("click", handleClick);
  }, []);

  // ---- Polling ------------------------------------------------------------
  const poll = useCallback(async () => {
    try {
      const state: string = await invoke("get_connection_state");
      setConnectionState(state);

      if (state === "disconnected" && viewRef.current !== "home") {
        setView("home");
        setMicEnabled(false);
        setCamEnabled(false);
        setMessages([]);
        setVideoFrames(new Map());
        setShowChat(false);
        setIsHandRaised(false);
        setUnreadCount(0);
        setHandRaisedMap({});
        return;
      }

      if (state === "connected" || state === "reconnecting") {
        const ps: Participant[] = await invoke("get_participants");
        setParticipants(ps);

        const ms: ChatMessage[] = await invoke("get_messages");
        setMessages(ms);
      }
    } catch (e) {
      console.error("poll error:", e);
    }
  }, []);

  useEffect(() => {
    if (view === "home") return;

    poll();
    const id = setInterval(poll, 1000);
    return () => clearInterval(id);
  }, [view, poll]);

  // ---- Video frame events -------------------------------------------------
  useEffect(() => {
    if (view === "home") return;

    let unlisten: UnlistenFn | null = null;

    listen<VideoFrame>("video-frame", (event) => {
      const { track_sid, data } = event.payload;
      setVideoFrames((prev) => {
        const next = new Map(prev);
        next.set(track_sid, data);
        return next;
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, [view]);

  // ---- Hand raise & unread events (Task 2.8) ------------------------------
  useEffect(() => {
    if (view === "home") return;

    let unlistenHand: UnlistenFn | null = null;
    let unlistenUnread: UnlistenFn | null = null;

    listen<{ participantSid: string; raised: boolean; position: number }>(
      "hand-raised-changed",
      (event) => {
        const { participantSid, raised, position } = event.payload;
        setHandRaisedMap((prev) => ({
          ...prev,
          [participantSid]: raised ? position : 0,
        }));
        // If our own hand was auto-lowered
        // We don't have localSid here, but we track via isHandRaised
        if (!raised) {
          // Check via invoke if our hand is still raised
          invoke<boolean>("is_hand_raised").then((val) => {
            setIsHandRaised(val);
          });
        }
      }
    ).then((fn) => {
      unlistenHand = fn;
    });

    listen<number>("unread-count-changed", (event) => {
      setUnreadCount(event.payload);
    }).then((fn) => {
      unlistenUnread = fn;
    });

    return () => {
      if (unlistenHand) unlistenHand();
      if (unlistenUnread) unlistenUnread();
    };
  }, [view]);

  // ---- Handlers -----------------------------------------------------------
  const handleJoin = () => {
    setView("call");
  };

  const handleToggleMic = async () => {
    const next = !micEnabled;
    setMicEnabled(next);
    try {
      await invoke("toggle_mic", { enabled: next });
    } catch (e) {
      console.error("mic toggle error:", e);
      setMicEnabled(!next);
    }
  };

  const handleToggleCam = async () => {
    const next = !camEnabled;
    setCamEnabled(next);
    try {
      await invoke("toggle_camera", { enabled: next });
    } catch (e) {
      console.error("camera toggle error:", e);
      setCamEnabled(!next);
    }
  };

  const handleHangUp = async () => {
    try {
      await invoke("disconnect");
    } catch (e) {
      console.error("disconnect error:", e);
    }
    setView("home");
    setMicEnabled(false);
    setCamEnabled(false);
    setMessages([]);
    setVideoFrames(new Map());
    setShowChat(false);
    setConnectionState("disconnected");
    setIsHandRaised(false);
    setUnreadCount(0);
    setHandRaisedMap({});
  };

  const handleToggleHandRaise = async () => {
    try {
      if (isHandRaised) {
        await invoke("lower_hand");
      } else {
        await invoke("raise_hand");
      }
      setIsHandRaised(!isHandRaised);
    } catch (e) {
      console.error("hand raise error:", e);
    }
  };

  const handleToggleChat = async () => {
    const newState = !showChat;
    setShowChat(newState);
    try {
      await invoke("set_chat_open", { open: newState });
    } catch (e) {
      console.error("set_chat_open error:", e);
    }
    if (newState) setUnreadCount(0);
  };

  const handleSendChat = async (text: string) => {
    try {
      await invoke("send_chat", { text });
    } catch (e) {
      console.error("send error:", e);
    }
  };

  // ---- Render -------------------------------------------------------------
  return (
    <I18nContext.Provider value={t}>
      <header>
        <h1>Visio Mobile</h1>
        <StatusBadge state={connectionState} />
      </header>
      <main>
        {view === "home" && (
          <HomeView
            onJoin={handleJoin}
            onOpenSettings={() => setShowSettings(true)}
            displayName={displayName}
            onDisplayNameChange={setDisplayName}
          />
        )}
        {view === "call" && (
          <CallView
            participants={participants}
            micEnabled={micEnabled}
            camEnabled={camEnabled}
            videoFrames={videoFrames}
            messages={messages}
            handRaisedMap={handRaisedMap}
            isHandRaised={isHandRaised}
            unreadCount={unreadCount}
            showChat={showChat}
            onToggleMic={handleToggleMic}
            onToggleCam={handleToggleCam}
            onHangUp={handleHangUp}
            onToggleHandRaise={handleToggleHandRaise}
            onToggleChat={handleToggleChat}
            onSendChat={handleSendChat}
            onShowMicPicker={() => {
              setShowMicPicker(!showMicPicker);
              setShowCamPicker(false);
            }}
            onShowCamPicker={() => {
              setShowCamPicker(!showCamPicker);
              setShowMicPicker(false);
            }}
            showMicPicker={showMicPicker}
            showCamPicker={showCamPicker}
            audioInputs={audioInputs}
            audioOutputs={audioOutputs}
            videoInputs={videoInputs}
            selectedAudioInput={selectedAudioInput}
            selectedVideoInput={selectedVideoInput}
            onSelectAudioInput={setSelectedAudioInput}
            onSelectVideoInput={setSelectedVideoInput}
          />
        )}
      </main>
      {showSettings && (
        <SettingsModal
          onClose={() => setShowSettings(false)}
          onLanguageChange={(l) => setLang(l)}
          onThemeChange={(t) => setTheme(t)}
          onDisplayNameChange={setDisplayName}
          initialDisplayName={displayName}
        />
      )}
    </I18nContext.Provider>
  );
}
