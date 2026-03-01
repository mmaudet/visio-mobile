import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type View = "home" | "call" | "chat";

interface Participant {
  sid: string;
  identity: string;
  name: string | null;
  is_muted: boolean;
  has_video: boolean;
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function getInitials(name: string | null | undefined): string {
  if (!name) return "?";
  const parts = name.trim().split(/\s+/);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return name.substring(0, 2).toUpperCase();
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
  return <span className={`status-badge ${state}`}>{state}</span>;
}

// -- Video Tile -------------------------------------------------------------

function VideoTile({ trackSid, frames }: { trackSid: string; frames: Map<string, string> }) {
  const src = frames.get(trackSid);
  if (!src) {
    return null;
  }
  return (
    <img
      className="video-frame"
      src={`data:image/jpeg;base64,${src}`}
      alt="video"
    />
  );
}

// -- Home View --------------------------------------------------------------

function HomeView({
  onJoin,
}: {
  onJoin: (meetUrl: string, username: string | null) => void;
}) {
  const [meetUrl, setMeetUrl] = useState("");
  const [username, setUsername] = useState("");
  const [error, setError] = useState("");
  const [joining, setJoining] = useState(false);

  const handleJoin = async () => {
    const url = meetUrl.trim();
    if (!url) {
      setError("Please enter a meeting URL");
      return;
    }
    setError("");
    setJoining(true);
    try {
      const uname = username.trim() || null;
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
      <div className="join-form">
        <h2>Join a Room</h2>
        <p>Enter a meeting URL and your display name</p>
        <div className="form-group">
          <label htmlFor="meetUrl">Meeting URL</label>
          <input
            id="meetUrl"
            type="text"
            placeholder="meet.example.com/my-room"
            autoComplete="off"
            value={meetUrl}
            onChange={(e) => setMeetUrl(e.target.value)}
            onKeyDown={handleKeyDown}
          />
        </div>
        <div className="form-group">
          <label htmlFor="username">Display Name (optional)</label>
          <input
            id="username"
            type="text"
            placeholder="Your name"
            autoComplete="off"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            onKeyDown={handleKeyDown}
          />
        </div>
        <button
          className="btn btn-primary"
          disabled={joining}
          onClick={handleJoin}
        >
          {joining ? "Connecting..." : "Join"}
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
  onToggleMic,
  onToggleCam,
  onHangUp,
  onOpenChat,
}: {
  participants: Participant[];
  micEnabled: boolean;
  camEnabled: boolean;
  videoFrames: Map<string, string>;
  onToggleMic: () => void;
  onToggleCam: () => void;
  onHangUp: () => void;
  onOpenChat: () => void;
}) {
  return (
    <div id="call" className="section active">
      <div className="call-content">
        <div className="section-label">Participants</div>
        {participants.length === 0 ? (
          <div className="empty-state">No other participants yet</div>
        ) : (
          <ul className="participant-list">
            {participants.map((p) => {
              const displayName = p.name || p.identity || "Unknown";
              return (
                <li key={p.sid} className="participant-item">
                  {p.has_video ? (
                    <div className="participant-video-wrapper">
                      <VideoTile trackSid={p.sid} frames={videoFrames} />
                      <div className="participant-avatar-overlay">
                        {getInitials(displayName)}
                      </div>
                    </div>
                  ) : (
                    <div className="participant-avatar">
                      {getInitials(displayName)}
                    </div>
                  )}
                  <div className="participant-info">
                    <div className="participant-name">{displayName}</div>
                    <div className="participant-status">
                      {p.connection_quality}
                    </div>
                  </div>
                  <div className="participant-icons">
                    {p.is_muted && (
                      <span className="icon-muted">Muted</span>
                    )}
                    {p.has_video && (
                      <span className="icon-video">Video</span>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
      <div className="call-controls">
        <button
          className={`ctrl-btn ${micEnabled ? "active" : "off"}`}
          title="Toggle microphone"
          onClick={onToggleMic}
        >
          Mic
        </button>
        <button
          className={`ctrl-btn ${camEnabled ? "active" : "off"}`}
          title="Toggle camera"
          onClick={onToggleCam}
        >
          Cam
        </button>
        <button
          className="ctrl-btn chat-toggle"
          title="Open chat"
          onClick={onOpenChat}
        >
          Chat
        </button>
        <button
          className="ctrl-btn hangup"
          title="Leave call"
          onClick={onHangUp}
        >
          End
        </button>
      </div>
    </div>
  );
}

// -- Chat View --------------------------------------------------------------

function ChatView({
  messages,
  onBack,
  onSend,
}: {
  messages: ChatMessage[];
  onBack: () => void;
  onSend: (text: string) => void;
}) {
  const [input, setInput] = useState("");
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [messages.length]);

  const handleSend = () => {
    const text = input.trim();
    if (!text) return;
    setInput("");
    onSend(text);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleSend();
  };

  return (
    <div id="chat" className="section active">
      <div className="chat-header">
        <button className="back-btn" onClick={onBack}>
          Back
        </button>
        <h3>Chat</h3>
        <span></span>
      </div>
      <div className="message-list" ref={listRef}>
        {messages.length === 0 ? (
          <div className="chat-empty">No messages yet</div>
        ) : (
          messages.map((m) => (
            <div key={m.id} className="message-item">
              <div className="message-sender">
                {m.sender_name || "Unknown"}
              </div>
              <div className="message-bubble">{m.text}</div>
              <div className="message-time">{formatTime(m.timestamp_ms)}</div>
            </div>
          ))
        )}
      </div>
      <div className="chat-input-area">
        <input
          type="text"
          placeholder="Type a message..."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
        />
        <button className="send-btn" onClick={handleSend}>
          Send
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

  const viewRef = useRef(view);
  viewRef.current = view;

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

    poll(); // immediate first poll
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
    setConnectionState("disconnected");
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
    <>
      <header>
        <h1>Visio</h1>
        <StatusBadge state={connectionState} />
      </header>
      <main>
        {view === "home" && <HomeView onJoin={handleJoin} />}
        {view === "call" && (
          <CallView
            participants={participants}
            micEnabled={micEnabled}
            camEnabled={camEnabled}
            videoFrames={videoFrames}
            onToggleMic={handleToggleMic}
            onToggleCam={handleToggleCam}
            onHangUp={handleHangUp}
            onOpenChat={() => setView("chat")}
          />
        )}
        {view === "chat" && (
          <ChatView
            messages={messages}
            onBack={() => setView("call")}
            onSend={handleSendChat}
          />
        )}
      </main>
    </>
  );
}
