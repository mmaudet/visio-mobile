// Mock Tauri APIs for browser-based development (npm run dev)
// This file is only loaded when running outside of Tauri

const mockSettings: {
  display_name: string | null;
  language: string | null;
  mic_enabled_on_join: boolean;
  camera_enabled_on_join: boolean;
  theme: string;
  meet_instances: string[];
} = {
  display_name: null,
  language: "fr",
  mic_enabled_on_join: true,
  camera_enabled_on_join: false,
  theme: "light",
  meet_instances: ["meet.linagora.com"],
};

const mockParticipants = [
  {
    sid: "local-123",
    identity: "test-user",
    name: "Test User", // Will be updated dynamically from mockSettings.display_name
    is_muted: false,
    has_video: false,
    video_track_sid: null,
    connection_quality: "Excellent",
  },
];

function getLocalParticipant() {
  return {
    ...mockParticipants[0],
    name: mockSettings.display_name || "Guest",
  };
}

let connectionState = "disconnected";

const mockInvoke = async (cmd: string, args?: Record<string, unknown>): Promise<unknown> => {
  console.log(`[Tauri Mock] invoke: ${cmd}`, args);

  switch (cmd) {
    case "get_settings":
      return mockSettings;
    case "get_meet_instances":
      return mockSettings.meet_instances;
    case "set_display_name":
      mockSettings.display_name = args?.name as string || null;
      return null;
    case "set_language":
      mockSettings.language = args?.lang as string || null;
      return null;
    case "set_theme":
      mockSettings.theme = args?.theme as string || "light";
      return null;
    case "set_mic_enabled_on_join":
      mockSettings.mic_enabled_on_join = args?.enabled as boolean;
      return null;
    case "set_camera_enabled_on_join":
      mockSettings.camera_enabled_on_join = args?.enabled as boolean;
      return null;
    case "set_meet_instances":
      mockSettings.meet_instances = args?.instances as string[];
      return null;
    case "validate_room":
      // Simulate room validation
      const url = args?.url as string;
      if (url?.includes("-")) {
        return { status: "valid", livekit_url: "wss://mock.livekit.cloud", token: "mock-token" };
      }
      return { status: "not_found" };
    case "connect":
      connectionState = "connected";
      return null;
    case "disconnect":
      connectionState = "disconnected";
      return null;
    case "get_connection_state":
      return connectionState;
    case "get_participants":
      return connectionState === "connected" ? mockParticipants.slice(1) : [];
    case "get_local_participant":
      return connectionState === "connected" ? getLocalParticipant() : null;
    case "get_messages":
      return [];
    case "toggle_mic":
      mockParticipants[0].is_muted = !(args?.enabled as boolean);
      return null;
    case "toggle_camera":
      mockParticipants[0].has_video = args?.enabled as boolean;
      return null;
    case "send_chat":
      console.log("[Mock] Chat sent:", args?.text);
      return null;
    case "raise_hand":
    case "lower_hand":
      return null;
    case "is_hand_raised":
      return false;
    case "set_chat_open":
      return null;
    case "get_all_sessions":
      return [];
    case "get_login_url":
      return `https://${args?.instance}/api/v1.0/authenticate/?silent=false&returnTo=visio://auth-callback`;
    case "open_url":
      // In browser mode, open in new tab
      window.open(args?.url as string, "_blank");
      return null;
    case "logout":
      return null;
    case "generate_random_slug":
      const chars = "abcdefghijklmnopqrstuvwxyz";
      const rand = (n: number) => Array.from({ length: n }, () => chars[Math.floor(Math.random() * 26)]).join("");
      return `${rand(3)}-${rand(4)}-${rand(3)}`;
    default:
      console.warn(`[Tauri Mock] Unknown command: ${cmd}`);
      return null;
  }
};

const mockListen = async (_event: string, _handler: (event: unknown) => void) => {
  // Return unsubscribe function
  return () => {};
};

const mockOnOpenUrl = async (_handler: (urls: string[]) => void) => {
  return () => {};
};

export function setupTauriMock() {
  const win = window as Window & { __TAURI_INTERNALS__?: unknown; __TAURI__?: unknown };
  if (typeof window !== "undefined" && !win.__TAURI_INTERNALS__) {
    console.log("[Tauri Mock] Running in browser mode - mocking Tauri APIs");

    // Mock the internal Tauri object
    win.__TAURI_INTERNALS__ = {
      invoke: mockInvoke,
      transformCallback: () => 0,
    };

    // Mock @tauri-apps/api/core
    win.__TAURI__ = {
      core: { invoke: mockInvoke },
      event: { listen: mockListen },
    };
  }
}

// For deep-link plugin mock
export const mockDeepLink = {
  onOpenUrl: mockOnOpenUrl,
};
