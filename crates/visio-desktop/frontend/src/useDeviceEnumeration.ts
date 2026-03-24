/**
 * useDeviceEnumeration — unified device enumeration hook.
 *
 * Replaces the three separate enumeration sites (lobby mount, in-call
 * picker, in-call refresh) with a single lazy hook that:
 *  - Enumerates audio input, output, AND video devices
 *  - Auto-selects the default device on first load
 *  - Falls back to the default device when the selected one disappears
 *  - Exposes a `devicesEnumerated` flag
 *  - Listens for `audio-devices-changed` and `audio-device-error` events
 *  - Does NOT enumerate until `enumerate()` is called (lazy — avoids
 *    the macOS mic permission issue, see #161)
 */

import { useState, useEffect, useRef, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ---------------------------------------------------------------------------
// Types (canonical — shared across lobby + in-call)
// ---------------------------------------------------------------------------

export interface NativeAudioDevice {
  name: string
  is_default: boolean
}

export interface NativeVideoDevice {
  name: string
  unique_id: string
  is_default: boolean
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Returns a fallback device name when the currently selected device
 * disappears. If the current selection is still present, returns it
 * unchanged.
 */
function resolveAudioFallback(
  prev: string,
  devices: NativeAudioDevice[],
  selectCommand: string
): string {
  if (!prev || devices.some((d) => d.name === prev)) return prev
  const def = devices.find((d) => d.is_default)
  const fallback = def ? def.name : (devices[0]?.name ?? '')
  invoke(selectCommand, { deviceName: fallback }).catch(() => {})
  return fallback
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export interface DeviceEnumerationState {
  audioInputs: NativeAudioDevice[]
  audioOutputs: NativeAudioDevice[]
  videoInputs: NativeVideoDevice[]
  selectedAudioInput: string
  selectedAudioOutput: string
  selectedVideoInput: string
  setSelectedAudioInput: React.Dispatch<React.SetStateAction<string>>
  setSelectedAudioOutput: React.Dispatch<React.SetStateAction<string>>
  setSelectedVideoInput: React.Dispatch<React.SetStateAction<string>>
  devicesEnumerated: boolean
  /** Trigger device enumeration. Safe to call multiple times. */
  enumerate: () => Promise<void>
}

export interface UseDeviceEnumerationOptions {
  /**
   * Called after the selected audio input falls back to a different
   * device (e.g. Bluetooth headset unplugged). Useful for restarting
   * mic preview in the lobby.
   */
  onInputFallback?: () => void
}

export function useDeviceEnumeration(
  options: UseDeviceEnumerationOptions = {}
): DeviceEnumerationState {
  const [audioInputs, setAudioInputs] = useState<NativeAudioDevice[]>([])
  const [audioOutputs, setAudioOutputs] = useState<NativeAudioDevice[]>([])
  const [videoInputs, setVideoInputs] = useState<NativeVideoDevice[]>([])
  const [selectedAudioInput, setSelectedAudioInput] = useState('')
  const [selectedAudioOutput, setSelectedAudioOutput] = useState('')
  const [selectedVideoInput, setSelectedVideoInput] = useState('')
  const [devicesEnumerated, setDevicesEnumerated] = useState(false)

  // Keep a stable ref to the callback so the listener doesn't go stale.
  const onInputFallbackRef = useRef(options.onInputFallback)
  onInputFallbackRef.current = options.onInputFallback

  // ---- Core enumerate function -------------------------------------------
  const enumerate = useCallback(async () => {
    try {
      const [inputs, outputs, cameras] = await Promise.all([
        invoke<NativeAudioDevice[]>('list_audio_input_devices'),
        invoke<NativeAudioDevice[]>('list_audio_output_devices'),
        invoke<NativeVideoDevice[]>('list_video_input_devices'),
      ])
      setAudioInputs(inputs)
      setAudioOutputs(outputs)
      setVideoInputs(cameras)
      setDevicesEnumerated(true)

      // Auto-select defaults when nothing is selected yet.
      setSelectedAudioInput((prev) => {
        if (prev) return prev
        const def = inputs.find((d) => d.is_default)
        return def ? def.name : ''
      })
      setSelectedAudioOutput((prev) => {
        if (prev) return prev
        const def = outputs.find((d) => d.is_default)
        return def ? def.name : ''
      })
      setSelectedVideoInput((prev) => {
        if (prev) return prev
        const def = cameras.find((d) => d.is_default)
        return def ? def.unique_id : ''
      })
    } catch (e) {
      console.warn('Device enumeration failed:', e)
    }
  }, [])

  // ---- Audio-device-changed listener (fallback logic) --------------------
  useEffect(() => {
    let unlistenChanged: (() => void) | null = null
    let unlistenError: (() => void) | null = null

    listen('audio-devices-changed', async () => {
      try {
        const [inputs, outputs] = await Promise.all([
          invoke<NativeAudioDevice[]>('list_audio_input_devices'),
          invoke<NativeAudioDevice[]>('list_audio_output_devices'),
        ])
        setAudioInputs(inputs)
        setAudioOutputs(outputs)

        setSelectedAudioInput((prev) => {
          const result = resolveAudioFallback(
            prev,
            inputs,
            'select_audio_input'
          )
          if (result !== prev) onInputFallbackRef.current?.()
          return result
        })

        setSelectedAudioOutput((prev) =>
          resolveAudioFallback(prev, outputs, 'select_audio_output')
        )
      } catch (e) {
        console.warn('Failed to re-enumerate audio devices after change:', e)
      }
    }).then((fn) => {
      unlistenChanged = fn
    })

    listen('audio-device-error', (event) => {
      console.warn('Audio device error:', event.payload)
      enumerate()
    }).then((fn) => {
      unlistenError = fn
    })

    return () => {
      unlistenChanged?.()
      unlistenError?.()
    }
  }, [enumerate])

  return {
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
    enumerate,
  }
}
