import AVFoundation
import SwiftUI

// MARK: - Audio Device Utilities

/// Shared audio device enumeration and helper functions used by both
/// InCallSettingsSheet (MicroTabContent) and CallView (AudioDeviceSheet).

/// Loads current audio input/output device state from AVAudioSession.
struct AudioDeviceSnapshot {
    var availableInputs: [AVAudioSessionPortDescription]
    var currentOutputs: [AVAudioSessionPortDescription]
    var currentInput: AVAudioSessionPortDescription?
    var isSpeakerOverride: Bool

    static func current() -> AudioDeviceSnapshot {
        let session = AVAudioSession.sharedInstance()
        let outputs = session.currentRoute.outputs
        return AudioDeviceSnapshot(
            availableInputs: session.availableInputs ?? [],
            currentOutputs: outputs,
            currentInput: session.currentRoute.inputs.first,
            isSpeakerOverride: outputs.contains { $0.portType == .builtInSpeaker }
        )
    }
}

/// Sets the preferred audio input port.
func selectAudioInput(_ port: AVAudioSessionPortDescription) {
    do { try AVAudioSession.sharedInstance().setPreferredInput(port) }
    catch { NSLog("Failed to set preferred input: %@", error.localizedDescription) }
}

/// Returns true if the port is an external output (Bluetooth, headphones, etc.)
func isExternalAudioOutput(_ port: AVAudioSessionPortDescription) -> Bool {
    switch port.portType {
    case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP,
         .headphones, .airPlay, .carAudio, .usbAudio:
        return true
    default:
        return false
    }
}

/// Returns an SF Symbol name for an audio output port.
func iconForAudioOutputPort(_ port: AVAudioSessionPortDescription) -> String {
    switch port.portType {
    case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP:
        return "wave.3.right"
    case .headphones:
        return "headphones"
    case .airPlay:
        return "airplayaudio"
    case .carAudio:
        return "car"
    default:
        return "speaker.wave.2"
    }
}

/// Returns an SF Symbol name for an audio input port.
func iconForAudioInputPort(_ port: AVAudioSessionPortDescription) -> String {
    switch port.portType {
    case .bluetoothHFP:
        return "wave.3.right"
    case .headsetMic:
        return "headphones"
    case .builtInMic:
        return "iphone"
    default:
        return "mic"
    }
}
