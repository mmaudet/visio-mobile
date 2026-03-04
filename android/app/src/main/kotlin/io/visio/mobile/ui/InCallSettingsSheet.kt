package io.visio.mobile.ui

import android.content.Context
import android.media.AudioDeviceInfo
import android.media.AudioManager
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.ui.graphics.vector.ImageVector
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
    onSelectAudioOutput: (AudioDeviceInfo) -> Unit,
    onSwitchCamera: (Boolean) -> Unit,
    isFrontCamera: Boolean
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
        containerColor = VisioColors.PrimaryDark75
    ) {
        // Title
        Text(
            text = Strings.t("settings.incall", lang),
            style = MaterialTheme.typography.titleMedium,
            color = VisioColors.White,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
        )

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp)
        ) {
            // Left sidebar: icon tabs
            Column(
                modifier = Modifier
                    .width(56.dp)
                    .padding(top = 8.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                TabIcon(
                    iconRes = R.drawable.ri_mic_line,
                    label = Strings.t("settings.incall.micro", lang),
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 }
                )
                TabIcon(
                    iconRes = R.drawable.ri_video_on_line,
                    label = Strings.t("settings.incall.camera", lang),
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 }
                )
                TabIcon(
                    icon = Icons.Outlined.Notifications,
                    label = Strings.t("settings.incall.notifications", lang),
                    selected = selectedTab == 2,
                    onClick = { selectedTab = 2 }
                )
            }

            // Right content
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(start = 8.dp, end = 8.dp, bottom = 32.dp)
            ) {
                when (selectedTab) {
                    0 -> MicroTab(context, lang, onSelectAudioOutput)
                    1 -> CameraTab(lang, isFrontCamera, onSwitchCamera)
                    2 -> NotificationsTab(
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
                        }
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
    onClick: () -> Unit
) {
    IconButton(
        onClick = onClick,
        modifier = Modifier
            .size(48.dp)
            .background(
                if (selected) VisioColors.Primary500 else VisioColors.PrimaryDark100,
                RoundedCornerShape(8.dp)
            )
    ) {
        Icon(
            painter = painterResource(iconRes),
            contentDescription = label,
            tint = VisioColors.White,
            modifier = Modifier.size(20.dp)
        )
    }
}

@Composable
private fun TabIcon(
    icon: ImageVector,
    label: String,
    selected: Boolean,
    onClick: () -> Unit
) {
    IconButton(
        onClick = onClick,
        modifier = Modifier
            .size(48.dp)
            .background(
                if (selected) VisioColors.Primary500 else VisioColors.PrimaryDark100,
                RoundedCornerShape(8.dp)
            )
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = VisioColors.White,
            modifier = Modifier.size(20.dp)
        )
    }
}

@Composable
private fun MicroTab(
    context: Context,
    lang: String,
    onSelectAudioOutput: (AudioDeviceInfo) -> Unit
) {
    val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    val inputDevices = remember {
        audioManager.getDevices(AudioManager.GET_DEVICES_INPUTS).filter {
            it.type in listOf(
                AudioDeviceInfo.TYPE_BUILTIN_MIC,
                AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
                AudioDeviceInfo.TYPE_USB_HEADSET,
                AudioDeviceInfo.TYPE_WIRED_HEADSET
            )
        }
    }

    val outputDevices = remember {
        audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS).filter {
            it.type in listOf(
                AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
                AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
                AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
                AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
                AudioDeviceInfo.TYPE_WIRED_HEADSET,
                AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
                AudioDeviceInfo.TYPE_USB_HEADSET
            )
        }
    }

    // Audio Input section
    SectionHeader(Strings.t("settings.incall.audioInput", lang))
    inputDevices.forEach { device ->
        val label = device.productName?.toString()?.ifBlank { null }
            ?: audioDeviceTypeName(device.type, lang)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = label,
                color = VisioColors.White,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f)
            )
        }
    }

    Spacer(modifier = Modifier.height(16.dp))

    // Audio Output section
    SectionHeader(Strings.t("settings.incall.audioOutput", lang))
    outputDevices.forEach { device ->
        val label = device.productName?.toString()?.ifBlank { null }
            ?: audioDeviceTypeName(device.type, lang)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onSelectAudioOutput(device) }
                .padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = label,
                color = VisioColors.White,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f)
            )
        }
    }
}

@Composable
private fun CameraTab(
    lang: String,
    isFrontCamera: Boolean,
    onSwitchCamera: (Boolean) -> Unit
) {
    var selectedFront by remember { mutableStateOf(isFrontCamera) }

    SectionHeader(Strings.t("settings.incall.cameraSelect", lang))

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable {
                selectedFront = true
                onSwitchCamera(true)
            }
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        RadioButton(
            selected = selectedFront,
            onClick = {
                selectedFront = true
                onSwitchCamera(true)
            },
            colors = RadioButtonDefaults.colors(
                selectedColor = VisioColors.Primary500,
                unselectedColor = VisioColors.White
            )
        )
        Text(
            text = Strings.t("settings.incall.cameraFront", lang),
            color = VisioColors.White,
            style = MaterialTheme.typography.bodyMedium
        )
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable {
                selectedFront = false
                onSwitchCamera(false)
            }
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        RadioButton(
            selected = !selectedFront,
            onClick = {
                selectedFront = false
                onSwitchCamera(false)
            },
            colors = RadioButtonDefaults.colors(
                selectedColor = VisioColors.Primary500,
                unselectedColor = VisioColors.White
            )
        )
        Text(
            text = Strings.t("settings.incall.cameraBack", lang),
            color = VisioColors.White,
            style = MaterialTheme.typography.bodyMedium
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
    onToggleMessage: (Boolean) -> Unit
) {
    SectionHeader(Strings.t("settings.incall.notifications", lang))

    NotificationRow(
        label = Strings.t("settings.incall.notifParticipant", lang),
        checked = notifParticipant,
        onToggle = onToggleParticipant
    )
    NotificationRow(
        label = Strings.t("settings.incall.notifHandRaised", lang),
        checked = notifHandRaised,
        onToggle = onToggleHandRaised
    )
    NotificationRow(
        label = Strings.t("settings.incall.notifMessage", lang),
        checked = notifMessage,
        onToggle = onToggleMessage
    )
}

@Composable
private fun NotificationRow(
    label: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            text = label,
            color = VisioColors.White,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(1f)
        )
        Switch(
            checked = checked,
            onCheckedChange = onToggle,
            colors = SwitchDefaults.colors(
                checkedTrackColor = VisioColors.Primary500,
                uncheckedTrackColor = VisioColors.PrimaryDark100
            )
        )
    }
}

@Composable
private fun SectionHeader(title: String) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleSmall,
        color = VisioColors.White,
        modifier = Modifier.padding(bottom = 8.dp)
    )
}

private fun audioDeviceTypeName(type: Int, lang: String): String = when (type) {
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
