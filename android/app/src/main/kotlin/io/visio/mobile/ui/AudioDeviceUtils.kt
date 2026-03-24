package io.visio.mobile.ui

import android.annotation.SuppressLint
import android.bluetooth.BluetoothManager
import android.content.Context
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import io.visio.mobile.ui.i18n.Strings

// ---------------------------------------------------------------------------
// Type constants
// ---------------------------------------------------------------------------

internal val BUILTIN_TYPES =
    setOf(
        AudioDeviceInfo.TYPE_BUILTIN_MIC,
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
    )

internal val BLUETOOTH_TYPES =
    setOf(
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
        AudioDeviceInfo.TYPE_BLE_HEADSET,
        AudioDeviceInfo.TYPE_BLE_SPEAKER,
    )

internal val INPUT_TYPES =
    setOf(
        AudioDeviceInfo.TYPE_BUILTIN_MIC,
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
        AudioDeviceInfo.TYPE_BLE_HEADSET,
        AudioDeviceInfo.TYPE_USB_HEADSET,
        AudioDeviceInfo.TYPE_WIRED_HEADSET,
    )

internal val OUTPUT_TYPES =
    setOf(
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
        AudioDeviceInfo.TYPE_BLE_HEADSET,
        AudioDeviceInfo.TYPE_BLE_SPEAKER,
        AudioDeviceInfo.TYPE_HEARING_AID,
        AudioDeviceInfo.TYPE_WIRED_HEADSET,
        AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
        AudioDeviceInfo.TYPE_USB_HEADSET,
        AudioDeviceInfo.TYPE_USB_DEVICE,
    )

// ---------------------------------------------------------------------------
// Filtering & deduplication
// ---------------------------------------------------------------------------

/**
 * Returns filtered and deduplicated input devices from the system.
 */
internal fun getFilteredInputDevices(audioManager: AudioManager): List<AudioDeviceInfo> {
    return audioManager.getDevices(AudioManager.GET_DEVICES_INPUTS)
        .filter { it.type in INPUT_TYPES }
        .deduplicateBluetooth()
}

/**
 * Returns filtered and deduplicated output devices from the system.
 */
internal fun getFilteredOutputDevices(audioManager: AudioManager): List<AudioDeviceInfo> {
    return audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
        .filter { it.type in OUTPUT_TYPES }
        .deduplicateBluetooth()
}

/**
 * Deduplicate Bluetooth devices: SCO + A2DP + BLE for the same physical
 * device appear as separate AudioDeviceInfo entries. Keep one per physical
 * device (prefer SCO for bidirectional audio), plus all non-Bluetooth
 * devices deduplicated by type.
 */
internal fun List<AudioDeviceInfo>.deduplicateBluetooth(): List<AudioDeviceInfo> {
    val (bt, nonBt) = partition { it.type in BLUETOOTH_TYPES }
    // Group Bluetooth devices by address (same physical device),
    // preferring SCO (bidirectional) over A2DP (output-only)
    val uniqueBt =
        bt.groupBy { it.address.ifBlank { "bt-${it.id}" } }
            .values
            .map { group ->
                group.firstOrNull { it.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO }
                    ?: group.first()
            }
    return nonBt.distinctBy { it.type } + uniqueBt
}

// ---------------------------------------------------------------------------
// Composable: audio device callback hook
// ---------------------------------------------------------------------------

/**
 * Data class holding the current device lists plus a refresh key that
 * increments on every device add/remove event.
 */
data class AudioDeviceState(
    val inputDevices: List<AudioDeviceInfo>,
    val outputDevices: List<AudioDeviceInfo>,
    val refreshKey: Int,
)

/**
 * Composable that registers an [AudioDeviceCallback] and returns a
 * reactive [AudioDeviceState] updated on device connect/disconnect.
 *
 * This replaces the duplicate DisposableEffect + callback registration
 * in both PreJoinScreen and InCallSettingsSheet.
 */
@Composable
internal fun rememberAudioDeviceState(audioManager: AudioManager): AudioDeviceState {
    var refreshKey by remember { mutableIntStateOf(0) }

    DisposableEffect(audioManager) {
        val callback =
            object : AudioDeviceCallback() {
                override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>?) {
                    refreshKey++
                }

                override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>?) {
                    refreshKey++
                }
            }
        audioManager.registerAudioDeviceCallback(
            callback,
            Handler(Looper.getMainLooper()),
        )
        onDispose {
            audioManager.unregisterAudioDeviceCallback(callback)
        }
    }

    val inputs = remember(refreshKey) { getFilteredInputDevices(audioManager) }
    val outputs = remember(refreshKey) { getFilteredOutputDevices(audioManager) }

    return AudioDeviceState(inputs, outputs, refreshKey)
}

/**
 * Returns the active communication device ID (API 31+), or null.
 */
internal fun getActiveCommunicationDeviceId(audioManager: AudioManager): Int? {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        audioManager.communicationDevice?.id
    } else {
        null
    }
}

// ---------------------------------------------------------------------------
// Labeling
// ---------------------------------------------------------------------------

/**
 * Returns a user-facing label for an audio device. Uses the Bluetooth
 * adapter name when available (most reliable for BT devices), then
 * falls back to AudioDeviceInfo.productName, then to a type-based label.
 */
internal fun audioDeviceLabel(
    context: Context,
    device: AudioDeviceInfo,
    lang: String,
): String {
    // Try Bluetooth adapter name first (most reliable for BT devices)
    resolveBluetoothName(context, device)?.let { return it }
    // Then try AudioDeviceInfo.productName for non-builtin devices
    if (device.type !in BUILTIN_TYPES) {
        val name = device.productName?.toString()?.ifBlank { null }
        if (name != null) return name
    }
    return audioDeviceTypeLabel(device.type, lang)
}

/**
 * Overload without context — used in InCallSettingsSheet where Bluetooth
 * adapter name resolution is not needed.
 */
internal fun audioDeviceLabel(
    device: AudioDeviceInfo,
    lang: String,
): String {
    if (device.type in BUILTIN_TYPES) {
        return audioDeviceTypeLabel(device.type, lang)
    }
    return device.productName?.toString()?.ifBlank { null }
        ?: audioDeviceTypeLabel(device.type, lang)
}

internal fun audioDeviceTypeLabel(
    type: Int,
    lang: String,
): String =
    when (type) {
        AudioDeviceInfo.TYPE_BUILTIN_MIC ->
            Strings.t("device.microphone", lang)
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER ->
            Strings.t("audio.speaker", lang)
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE ->
            Strings.t("audio.earpiece", lang)
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
        ->
            Strings.t("audio.bluetooth", lang)
        AudioDeviceInfo.TYPE_BLE_HEADSET,
        AudioDeviceInfo.TYPE_BLE_SPEAKER,
        ->
            Strings.t("audio.bluetooth", lang)
        AudioDeviceInfo.TYPE_HEARING_AID ->
            Strings.t("audio.hearingAid", lang)
        AudioDeviceInfo.TYPE_WIRED_HEADSET,
        AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
        ->
            Strings.t("audio.wiredHeadset", lang)
        AudioDeviceInfo.TYPE_USB_HEADSET,
        AudioDeviceInfo.TYPE_USB_DEVICE,
        ->
            Strings.t("audio.usbHeadset", lang)
        else -> "Audio"
    }

/**
 * Resolve the Bluetooth device name from BluetoothAdapter.bondedDevices.
 * AudioDeviceInfo.productName is often empty or generic for BT devices.
 */
@SuppressLint("MissingPermission")
internal fun resolveBluetoothName(
    context: Context,
    device: AudioDeviceInfo,
): String? {
    if (device.type !in BLUETOOTH_TYPES) return null
    val btManager =
        context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
            ?: return null
    val adapter = btManager.adapter ?: return null
    // Match by address if available
    val address = device.address
    if (address.isNotBlank()) {
        try {
            adapter.bondedDevices?.firstOrNull { it.address == address }
                ?.name?.let { return it }
        } catch (_: SecurityException) {
            // BLUETOOTH_CONNECT not granted
        }
    }
    // Fallback: match by product name
    val productName = device.productName?.toString()?.ifBlank { null }
    if (productName != null) {
        try {
            adapter.bondedDevices?.firstOrNull {
                it.name?.equals(productName, ignoreCase = true) == true
            }?.name?.let { return it }
        } catch (_: SecurityException) {
            // BLUETOOTH_CONNECT not granted
        }
    }
    return null
}
