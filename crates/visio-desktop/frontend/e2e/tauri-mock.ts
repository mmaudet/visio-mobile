import { type Page } from '@playwright/test';

/** Default mock state for a connected call */
export interface MockCallState {
  participants?: Array<{
    sid: string;
    identity: string;
    name: string;
    is_local: boolean;
    is_muted: boolean;
    has_video: boolean;
    video_track_sid: string | null;
    has_screen_share: boolean;
    screen_share_track_sid: string | null;
    connection_quality: string;
  }>;
  messages?: Array<{
    id: string;
    sender_sid: string;
    sender_name: string | null;
    text: string;
    timestamp_ms: number;
  }>;
  audioInputDevices?: Array<{ name: string; is_default: boolean }>;
  audioOutputDevices?: Array<{ name: string; is_default: boolean }>;
  videoInputDevices?: Array<{
    name: string;
    unique_id: string;
    is_default: boolean;
  }>;
  screenSources?: Array<{
    id: string;
    name: string;
    source_type: string;
    width: number;
    height: number;
  }>;
  connectionState?: string;
  /** When true, validate_room returns auth_required until a successful OIDC exchange */
  roomRequiresAuth?: boolean;
  /** When true, launch_oidc_browser rejects (e.g. the system browser cannot be opened) */
  oidcLaunchFails?: boolean;
  /** When true, exchange_pkce_code rejects (e.g. expired or invalid code) */
  oidcExchangeFails?: boolean;
}

const defaultState: MockCallState = {
  participants: [
    {
      sid: 'PA_local',
      identity: 'test-user',
      name: 'Test User',
      is_local: true,
      is_muted: false,
      has_video: true,
      video_track_sid: null,
      has_screen_share: false,
      screen_share_track_sid: null,
      connection_quality: 'Excellent',
    },
    {
      sid: 'PA_remote1',
      identity: 'bot',
      name: 'E2E Bot',
      is_local: false,
      is_muted: false,
      has_video: true,
      video_track_sid: null,
      has_screen_share: false,
      screen_share_track_sid: null,
      connection_quality: 'Good',
    },
  ],
  messages: [],
  audioInputDevices: [
    { name: 'Built-in Microphone', is_default: true },
    { name: 'External USB Mic', is_default: false },
  ],
  audioOutputDevices: [
    { name: 'Built-in Speakers', is_default: true },
    { name: 'Headphones', is_default: false },
    { name: 'Bluetooth Speaker', is_default: false },
  ],
  videoInputDevices: [
    { name: 'FaceTime HD Camera', unique_id: 'cam-1', is_default: true },
    { name: 'External Webcam', unique_id: 'cam-2', is_default: false },
  ],
  screenSources: [
    {
      id: 'screen-1',
      name: 'Main Display',
      source_type: 'monitor',
      width: 1920,
      height: 1080,
    },
    {
      id: 'window-1',
      name: 'Browser',
      source_type: 'window',
      width: 1280,
      height: 720,
    },
    {
      id: 'window-2',
      name: 'Terminal',
      source_type: 'window',
      width: 800,
      height: 600,
    },
  ],
  connectionState: 'connected',
};

/**
 * Inject Tauri invoke mock into the page BEFORE navigation.
 * This simulates the Tauri runtime so that @tauri-apps/api/core invoke()
 * and @tauri-apps/api/event listen() work without the real Tauri backend.
 */
export async function mockTauriCall(
  page: Page,
  overrides: Partial<MockCallState> = {},
) {
  const state = { ...defaultState, ...overrides };

  await page.addInitScript((stateJson: string) => {
    const state = JSON.parse(stateJson);
    let micEnabled = true;
    let cameraEnabled = true;
    let isScreenSharing = false;
    let handRaised = false;
    let oidcAuthenticated = false;
    const chatMessages = [...state.messages];

    // Callback registry (mimics Tauri's transformCallback system)
    const callbacks = new Map<number, (data: unknown) => void>();
    let nextId = 1;

    function registerCallback(
      callback?: (data: unknown) => void,
      once = false,
    ): number {
      const id = nextId++;
      if (callback) {
        callbacks.set(id, (data: unknown) => {
          if (once) callbacks.delete(id);
          callback(data);
        });
      }
      return id;
    }

    function unregisterCallback(id: number) {
      callbacks.delete(id);
    }

    // Event listener registry for plugin:event|listen mocking
    const eventListeners = new Map<string, number[]>();

    // Record of all invoke() calls, for test assertions
    (window as any).__invokeLog = [] as Array<{ cmd: string; args: any }>;

    // Test helper: deliver an event to registered listeners (mimics Tauri emit)
    (window as any).__emitTauriEvent = (event: string, payload: unknown) => {
      const ids = eventListeners.get(event) || [];
      for (const id of ids) {
        const cb = callbacks.get(id);
        if (cb) cb({ event, payload });
      }
    };

    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args?: any) => {
        (window as any).__invokeLog.push({ cmd, args });
        // Handle event plugin commands used by @tauri-apps/api/event listen()
        if (cmd === 'plugin:event|listen') {
          const event = args?.event;
          const handler = args?.handler;
          if (event && handler != null) {
            if (!eventListeners.has(event)) {
              eventListeners.set(event, []);
            }
            eventListeners.get(event)!.push(handler);
          }
          return handler;
        }
        if (cmd === 'plugin:event|unlisten') {
          const event = args?.event;
          const id = args?.id;
          if (event && eventListeners.has(event)) {
            const arr = eventListeners.get(event)!;
            const idx = arr.indexOf(id);
            if (idx !== -1) arr.splice(idx, 1);
          }
          unregisterCallback(id);
          return;
        }
        if (cmd === 'plugin:event|emit') {
          return null;
        }

        // Handle deep-link plugin
        if (cmd.startsWith('plugin:deep-link|')) {
          return null;
        }

        // Handle path plugin (resolveResource)
        if (cmd.startsWith('plugin:path|')) {
          return '/mock/path';
        }

        switch (cmd) {
          case 'get_connection_state':
            return state.connectionState;
          case 'get_participants':
            return state.participants.filter((p: any) => !p.is_local);
          case 'get_local_participant':
            return (
              state.participants.find((p: any) => p.is_local) || null
            );
          case 'get_messages':
            return chatMessages;
          case 'list_audio_input_devices':
            return state.audioInputDevices;
          case 'list_audio_output_devices':
            return state.audioOutputDevices;
          case 'list_video_input_devices':
            return state.videoInputDevices;
          case 'list_screen_sources':
            return state.screenSources;
          case 'toggle_mic':
            micEnabled = args?.enabled ?? !micEnabled;
            return;
          case 'toggle_camera':
            cameraEnabled = args?.enabled ?? !cameraEnabled;
            return;
          case 'select_audio_input':
            return;
          case 'select_audio_output':
            return;
          case 'select_video_input':
            return;
          case 'start_screen_share':
            isScreenSharing = true;
            return;
          case 'stop_screen_share':
            isScreenSharing = false;
            return;
          case 'send_chat':
            chatMessages.push({
              id: `msg-${chatMessages.length}`,
              text: args?.text || '',
              sender_sid: 'PA_local',
              sender_name: 'Test User',
              timestamp_ms: Date.now(),
            });
            return;
          case 'set_chat_open':
            return;
          case 'raise_hand':
            handRaised = true;
            return;
          case 'lower_hand':
            handRaised = false;
            return;
          case 'is_hand_raised':
            return handRaised;
          case 'send_reaction':
            return;
          case 'disconnect':
            return;
          case 'connect':
            // The real backend emits "connection-state-changed: connected"
            // once the LiveKit connection succeeds; PreJoinScreen only opens
            // the CallView after receiving it. Listeners are registered
            // before connect() is invoked, so a 0ms delay is enough.
            setTimeout(() => {
              (window as any).__emitTauriEvent(
                'connection-state-changed',
                'connected',
              );
            }, 0);
            return;
          case 'set_display_name':
            return;
          case 'get_settings':
            return {
              display_name: 'Test User',
              language: 'en',
              mic_enabled_on_join: true,
              camera_enabled_on_join: true,
              theme: 'light',
              adaptive_mode_enabled: false,
            };
          case 'set_settings':
            return;
          case 'get_meet_instances':
            return [];
          case 'get_session_state':
            return { state: 'unauthenticated' };
          case 'get_visio_history':
            return [];
          case 'get_upcoming_meetings':
            return [];
          case 'is_oidc_enabled':
            return true;
          case 'validate_room':
            if (state.roomRequiresAuth && !oidcAuthenticated) {
              return { status: 'auth_required' };
            }
            return {
              status: 'valid',
              livekit_url: 'ws://localhost:7880',
              token: 'fake-token',
              room_name: 'test-room',
            };
          case 'launch_oidc_browser':
            if (state.oidcLaunchFails) throw new Error('cannot open browser');
            return;
          case 'exchange_pkce_code':
            if (state.oidcExchangeFails) throw new Error('exchange failed');
            oidcAuthenticated = true;
            return {
              display_name: 'OIDC User',
              email: 'oidc@example.com',
              meet_instance: args?.meetInstance,
            };
          case 'load_blur_model':
            return;
          case 'get_background_mode':
            return 'off';
          case 'set_background_mode':
            return;
          case 'admit_participant':
            return;
          case 'deny_participant':
            return;
          case 'cancel_lobby':
            return;
          default:
            console.warn(`[tauri-mock] unhandled invoke: ${cmd}`, args);
            return null;
        }
      },

      transformCallback: registerCallback,
      unregisterCallback,
      runCallback: (id: number, data: unknown) => {
        const cb = callbacks.get(id);
        if (cb) cb(data);
      },
      callbacks,

      convertFileSrc: (path: string) => path,

      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { windowLabel: 'main', label: 'main' },
      },
    };

    // Also set up event plugin internals
    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (event: string, id: number) => {
        unregisterCallback(id);
      },
    };
  }, JSON.stringify(state));
}

/**
 * Navigate to home and join a room (triggering CallView with mock).
 */
export async function joinMockRoom(
  page: Page,
  overrides: Partial<MockCallState> = {},
) {
  await mockTauriCall(page, overrides);
  await page.goto('/');

  // Fill in room URL and join
  const urlInput = page.getByTestId('home-room-url-input');
  await urlInput.fill('https://meet.example.com/abc-defg-hij');

  // Wait for room validation (the mock returns "valid" after 500ms debounce)
  await page.getByTestId('home-join-button').waitFor({ state: 'attached', timeout: 5000 });

  // Wait until the button is enabled (room status becomes "valid")
  await page.waitForFunction(() => {
    const btn = document.querySelector('[data-testid="home-join-button"]') as HTMLButtonElement;
    return btn && !btn.disabled;
  }, undefined, { timeout: 5000 });

  const nameInput = page.getByTestId('home-display-name-input');
  await nameInput.fill('Test User');

  await page.getByTestId('home-join-button').click();

  // The app shows a pre-join (lobby) screen before entering the call
  await page.getByTestId('prejoin-join-button').click();

  // Wait for call view to render
  await page.getByTestId('call-mic-button').waitFor({ timeout: 5000 });
}
