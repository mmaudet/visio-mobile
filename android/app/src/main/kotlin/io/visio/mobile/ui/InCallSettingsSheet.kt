package io.visio.mobile.ui

import android.content.Context
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InCallSettingsSheet(
    initialTab: Int = 0,
    onDismiss: () -> Unit,
    onSelectAudioInput: (AudioDeviceInfo) -> Unit,
    onSelectAudioOutput: (AudioDeviceInfo) -> Unit,
    onSwitchCamera: (Boolean) -> Unit,
    isFrontCamera: Boolean,
) {
    val context = LocalContext.current
    val lang = VisioManager.currentLang
    val sheetState = rememberModalBottomSheetState()
    var selectedTab by remember { mutableIntStateOf(initialTab) }

    val settings = remember { VisioManager.client.getSettings() }
    var notifParticipant by remember { mutableStateOf(settings.notificationParticipantJoin) }
    var notifHandRaised by remember { mutableStateOf(settings.notificationHandRaised) }
    var notifMessage by remember { mutableStateOf(settings.notificationMessageReceived) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = VisioColors.PrimaryDark75,
    ) {
        // Title
        Text(
            text = Strings.t("settings.incall", lang),
            style = MaterialTheme.typography.titleMedium,
            color = VisioColors.White,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )

        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp),
        ) {
            // Left sidebar: icon tabs
            Column(
                modifier =
                    Modifier
                        .width(56.dp)
                        .padding(top = 8.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                TabIcon(
                    iconRes = R.drawable.ri_mic_line,
                    label = Strings.t("settings.incall.micro", lang),
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                )
                TabIcon(
                    iconRes = R.drawable.ri_video_on_line,
                    label = Strings.t("settings.incall.camera", lang),
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                )
                TabIcon(
                    icon = Icons.Outlined.Notifications,
                    label = Strings.t("settings.incall.notifications", lang),
                    selected = selectedTab == 2,
                    onClick = { selectedTab = 2 },
                )
            }

            // Right content
            Column(
                modifier =
                    Modifier
                        .weight(1f)
                        .padding(start = 8.dp, end = 8.dp, bottom = 32.dp),
            ) {
                when (selectedTab) {
                    0 -> MicroTab(context, lang, onSelectAudioInput, onSelectAudioOutput)
                    1 -> CameraTab(lang, isFrontCamera, onSwitchCamera)
                    2 ->
                        NotificationsTab(
                            lang = lang,
                            notifParticipant = notifParticipant,
                            notifHandRaised = notifHandRaised,
                            notifMessage = notifMessage,
                            onToggleParticipant = { enabled ->
                                notifParticipant = enabled
                                VisioManager.client.setNotificationParticipantJoin(enabled)
                            },
                            onToggleHandRaised = { enabled ->
                                notifHandRaised = enabled
                                VisioManager.client.setNotificationHandRaised(enabled)
                            },
                            onToggleMessage = { enabled ->
                                notifMessage = enabled
                                VisioManager.client.setNotificationMessageReceived(enabled)
                            },
                        )
                }
            }
        }
    }
}

@Composable
private fun TabIcon(
    iconRes: Int,
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    IconButton(
        onClick = onClick,
        modifier =
            Modifier
                .size(48.dp)
                .background(
                    if (selected) VisioColors.Primary500 else VisioColors.PrimaryDark100,
                    RoundedCornerShape(8.dp),
                ),
    ) {
        Icon(
            painter = painterResource(iconRes),
            contentDescription = label,
            tint = VisioColors.White,
            modifier = Modifier.size(20.dp),
        )
    }
}

@Composable
private fun TabIcon(
    icon: ImageVector,
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    IconButton(
        onClick = onClick,
        modifier =
            Modifier
                .size(48.dp)
                .background(
                    if (selected) VisioColors.Primary500 else VisioColors.PrimaryDark100,
                    RoundedCornerShape(8.dp),
                ),
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = VisioColors.White,
            modifier = Modifier.size(20.dp),
        )
    }
}

@Composable
private fun MicroTab(
    context: Context,
    lang: String,
    onSelectAudioInput: (AudioDeviceInfo) -> Unit,
    onSelectAudioOutput: (AudioDeviceInfo) -> Unit,
) {
    val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    var inputDevices by remember { mutableStateOf(getFilteredInputDevices(audioManager)) }
    var outputDevices by remember { mutableStateOf(getFilteredOutputDevices(audioManager)) }

    // Track active input and output devices independently
    var activeInputDeviceId by remember {
        mutableStateOf(
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                audioManager.communicationDevice?.id
            } else {
                null
            },
        )
    }
    var activeOutputDeviceId by remember {
        mutableStateOf(
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                audioManager.communicationDevice?.id
            } else {
                null
            },
        )
    }

    // React to device connect/disconnect events
    DisposableEffect(audioManager) {
        val callback =
            object : AudioDeviceCallback() {
                override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>?) {
                    inputDevices = getFilteredInputDevices(audioManager)
                    outputDevices = getFilteredOutputDevices(audioManager)
                }

                override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>?) {
                    inputDevices = getFilteredInputDevices(audioManager)
                    outputDevices = getFilteredOutputDevices(audioManager)
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        val commId = audioManager.communicationDevice?.id
                        activeInputDeviceId = commId
                        activeOutputDeviceId = commId
                    }
                }
            }
        audioManager.registerAudioDeviceCallback(callback, Handler(Looper.getMainLooper()))
        onDispose {
            audioManager.unregisterAudioDeviceCallback(callback)
        }
    }

    // Resolve which input is active: match by device ID, or for built-in mic
    // check if the communication device is also built-in (speaker/earpiece).
    fun isInputActive(device: AudioDeviceInfo): Boolean {
        if (activeInputDeviceId != null) {
            if (activeInputDeviceId == device.id) return true
            // Built-in mic is active when selected input is also built-in
            if (device.type == AudioDeviceInfo.TYPE_BUILTIN_MIC) {
                val selectedInput = inputDevices.find { it.id == activeInputDeviceId }
                if (selectedInput == null || selectedInput.type in BUILTIN_TYPES) return true
            }
            return false
        }
        val commDevice =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                audioManager.communicationDevice
            } else {
                return false
            }
        if (commDevice == null) return device.type == AudioDeviceInfo.TYPE_BUILTIN_MIC
        if (commDevice.id == device.id) return true
        if (device.type == AudioDeviceInfo.TYPE_BUILTIN_MIC && commDevice.type in BUILTIN_TYPES) return true
        return false
    }

    // Audio Input section
    SectionHeader(Strings.t("settings.incall.audioInput", lang))
    inputDevices.forEach { device ->
        val label = audioDeviceLabel(device, lang)
        val isActive = isInputActive(device)
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .clickable {
                        onSelectAudioInput(device)
                        activeInputDeviceId = device.id
                    }
                    .padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(
                selected = isActive,
                onClick = {
                    onSelectAudioInput(device)
                    activeInputDeviceId = device.id
                },
                colors =
                    RadioButtonDefaults.colors(
                        selectedColor = VisioColors.Primary500,
                        unselectedColor = VisioColors.White,
                    ),
            )
            Text(
                text = label,
                color = VisioColors.White,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
        }
    }

    Spacer(modifier = Modifier.height(16.dp))

    // Audio Output section
    SectionHeader(Strings.t("settings.incall.audioOutput", lang))
    outputDevices.forEach { device ->
        val label = audioDeviceLabel(device, lang)
        val isActive = activeOutputDeviceId == device.id
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .clickable {
                        onSelectAudioOutput(device)
                        activeOutputDeviceId = device.id
                    }
                    .padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(
                selected = isActive,
                onClick = {
                    onSelectAudioOutput(device)
                    activeOutputDeviceId = device.id
                },
                colors =
                    RadioButtonDefaults.colors(
                        selectedColor = VisioColors.Primary500,
                        unselectedColor = VisioColors.White,
                    ),
            )
            Text(
                text = label,
                color = VisioColors.White,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
        }
    }
}

@Composable
private fun CameraTab(
    lang: String,
    isFrontCamera: Boolean,
    onSwitchCamera: (Boolean) -> Unit,
) {
    var selectedFront by remember { mutableStateOf(isFrontCamera) }

    SectionHeader(Strings.t("settings.incall.cameraSelect", lang))

    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable {
                    selectedFront = true
                    onSwitchCamera(true)
                }
                .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(
            selected = selectedFront,
            onClick = {
                selectedFront = true
                onSwitchCamera(true)
            },
            colors =
                RadioButtonDefaults.colors(
                    selectedColor = VisioColors.Primary500,
                    unselectedColor = VisioColors.White,
                ),
        )
        Text(
            text = Strings.t("settings.incall.cameraFront", lang),
            color = VisioColors.White,
            style = MaterialTheme.typography.bodyMedium,
        )
    }

    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable {
                    selectedFront = false
                    onSwitchCamera(false)
                }
                .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(
            selected = !selectedFront,
            onClick = {
                selectedFront = false
                onSwitchCamera(false)
            },
            colors =
                RadioButtonDefaults.colors(
                    selectedColor = VisioColors.Primary500,
                    unselectedColor = VisioColors.White,
                ),
        )
        Text(
            text = Strings.t("settings.incall.cameraBack", lang),
            color = VisioColors.White,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

@Composable
private fun NotificationsTab(
    lang: String,
    notifParticipant: Boolean,
    notifHandRaised: Boolean,
    notifMessage: Boolean,
    onToggleParticipant: (Boolean) -> Unit,
    onToggleHandRaised: (Boolean) -> Unit,
    onToggleMessage: (Boolean) -> Unit,
) {
    SectionHeader(Strings.t("settings.incall.notifications", lang))

    NotificationRow(
        label = Strings.t("settings.incall.notifParticipant", lang),
        checked = notifParticipant,
        onToggle = onToggleParticipant,
    )
    NotificationRow(
        label = Strings.t("settings.incall.notifHandRaised", lang),
        checked = notifHandRaised,
        onToggle = onToggleHandRaised,
    )
    NotificationRow(
        label = Strings.t("settings.incall.notifMessage", lang),
        checked = notifMessage,
        onToggle = onToggleMessage,
    )
}

@Composable
private fun NotificationRow(
    label: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = label,
            color = VisioColors.White,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(1f),
        )
        Switch(
            checked = checked,
            onCheckedChange = onToggle,
            colors =
                SwitchDefaults.colors(
                    checkedTrackColor = VisioColors.Primary500,
                    uncheckedTrackColor = VisioColors.PrimaryDark100,
                ),
        )
    }
}

@Composable
private fun SectionHeader(title: String) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleSmall,
        color = VisioColors.White,
        modifier = Modifier.padding(bottom = 8.dp),
    )
}

private val BUILTIN_TYPES =
    setOf(
        AudioDeviceInfo.TYPE_BUILTIN_MIC,
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
    )

private val BLUETOOTH_TYPES =
    setOf(
        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
    )

private val INPUT_TYPES =
    listOf(
        AudioDeviceInfo.TYPE_BUILTIN_MIC,
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
        AudioDeviceInfo.TYPE_USB_HEADSET,
        AudioDeviceInfo.TYPE_WIRED_HEADSET,
    )

private val OUTPUT_TYPES =
    listOf(
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
        AudioDeviceInfo.TYPE_WIRED_HEADSET,
        AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
        AudioDeviceInfo.TYPE_USB_HEADSET,
    )

private fun getFilteredInputDevices(audioManager: AudioManager): List<AudioDeviceInfo> {
    val seenBuiltinTypes = mutableSetOf<Int>()
    return audioManager.getDevices(AudioManager.GET_DEVICES_INPUTS)
        .filter { it.type in INPUT_TYPES }
        .filter { device ->
            if (device.type in BUILTIN_TYPES) seenBuiltinTypes.add(device.type) else true
        }
}

private fun getFilteredOutputDevices(audioManager: AudioManager): List<AudioDeviceInfo> {
    val seenBuiltinTypes = mutableSetOf<Int>()
    val seenBtNames = mutableSetOf<String>()
    return audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
        .filter { it.type in OUTPUT_TYPES }
        // Dedup built-in devices (multiple mics/speakers reported by system)
        .filter { device ->
            if (device.type in BUILTIN_TYPES) seenBuiltinTypes.add(device.type) else true
        }
        // Dedup Bluetooth: A2DP and SCO often report the same headset.
        // Keep SCO (communication profile) and drop A2DP duplicates.
        .filter { device ->
            if (device.type in BLUETOOTH_TYPES) {
                val name = device.productName?.toString() ?: ""
                // SCO always passes; A2DP only if no SCO with same name was seen
                if (device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO) {
                    seenBtNames.add(name)
                    true
                } else {
                    !seenBtNames.contains(name).also { seenBtNames.add(name) }
                }
            } else {
                true
            }
        }
}

private fun audioDeviceLabel(
    device: AudioDeviceInfo,
    lang: String,
): String {
    return if (device.type in BUILTIN_TYPES) {
        audioDeviceTypeName(device.type, lang)
    } else {
        device.productName?.toString()?.ifBlank { null }
            ?: audioDeviceTypeName(device.type, lang)
    }
}

private fun audioDeviceTypeName(
    type: Int,
    lang: String,
): String =
    when (type) {
        AudioDeviceInfo.TYPE_BUILTIN_MIC -> Strings.t("device.microphone", lang)
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> Strings.t("audio.speaker", lang)
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> Strings.t("audio.earpiece", lang)
        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> Strings.t("audio.bluetooth", lang)
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO -> Strings.t("audio.bluetooth", lang)
        AudioDeviceInfo.TYPE_WIRED_HEADSET -> Strings.t("audio.wiredHeadset", lang)
        AudioDeviceInfo.TYPE_WIRED_HEADPHONES -> Strings.t("audio.wiredHeadphones", lang)
        AudioDeviceInfo.TYPE_USB_HEADSET -> Strings.t("audio.usbHeadset", lang)
        else -> Strings.t("audio.device", lang)
    }
