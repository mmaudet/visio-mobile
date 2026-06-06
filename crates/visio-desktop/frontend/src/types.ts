// Shared types — mirror the JSON shapes returned by Tauri commands defined in
// `crates/visio-desktop/src/lib.rs`. Keep them in sync with the Rust side.

export interface Participant {
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

export interface ChatMessage {
  id: string
  sender_sid: string
  sender_name: string | null
  text: string
  timestamp_ms: number
  encrypted: boolean
  decryption_failed: boolean
}

export interface Meeting {
  id: string
  summary: string
  start_time: number
  end_time: number
  room_url: string
  deep_link: string
  server_name: string
}

export interface VisioHistoryEntry {
  url: string
  display_name: string | null
}

export interface Settings {
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

export interface SessionStateAuthenticated {
  state: 'authenticated'
  display_name?: string
  email?: string
  meet_instance?: string
}
export interface SessionStateAnonymous {
  state: 'anonymous'
}
export type SessionStatePayload =
  | SessionStateAuthenticated
  | SessionStateAnonymous

export interface ScreenSource {
  id: string
  name: string
  source_type: string
  width: number
  height: number
  thumbnail: string
}
