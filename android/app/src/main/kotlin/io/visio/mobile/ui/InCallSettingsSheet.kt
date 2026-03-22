package io.visio.mobile.ui

import android.content.Context
import android.graphics.BitmapFactory
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.Language
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material.icons.outlined.People
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.visio.UserSearchResult

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InCallSettingsSheet(
    roomUrl: String,
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
                    icon = Icons.Outlined.Info,
                    label = Strings.t("settings.incall.roomInfo", lang),
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    modifier = Modifier.testTag("incall_tab_info"),
                )
                TabIcon(
                    iconRes = R.drawable.ri_mic_line,
                    label = Strings.t("settings.incall.micro", lang),
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    modifier = Modifier.testTag("incall_tab_microphone"),
                )
                TabIcon(
                    iconRes = R.drawable.ri_video_on_line,
                    label = Strings.t("settings.incall.camera", lang),
                    selected = selectedTab == 2,
                    onClick = { selectedTab = 2 },
                    modifier = Modifier.testTag("incall_tab_camera"),
                )
                TabIcon(
                    icon = Icons.Outlined.Notifications,
                    label = Strings.t("settings.incall.notifications", lang),
                    selected = selectedTab == 3,
                    onClick = { selectedTab = 3 },
                    modifier = Modifier.testTag("incall_tab_notifications"),
                )
                if (VisioManager.currentAccessLevel == "restricted") {
                    TabIcon(
                        icon = Icons.Outlined.People,
                        label = Strings.t("restricted.members", lang),
                        selected = selectedTab == 4,
                        onClick = { selectedTab = 4 },
                    )
                }
            }

            // Right content
            Column(
                modifier =
                    Modifier
                        .weight(1f)
                        .padding(start = 8.dp, end = 8.dp, bottom = 32.dp),
            ) {
                when (selectedTab) {
                    0 -> RoomInfoTab(roomUrl, lang)
                    1 -> MicroTab(context, lang, onSelectAudioInput, onSelectAudioOutput)
                    2 -> CameraTab(lang, isFrontCamera, onSwitchCamera)
                    3 ->
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
                    4 -> MembersTab(lang = lang)
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
    modifier: Modifier = Modifier,
) {
    IconButton(
        onClick = onClick,
        modifier =
            modifier
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
    modifier: Modifier = Modifier,
) {
    IconButton(
        onClick = onClick,
        modifier =
            modifier
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
    AudioDeviceList(
        devices = inputDevices,
        lang = lang,
        testTagPrefix = "incall_audio_input",
        isActive = { device -> isInputActive(device) },
        onSelect = { device ->
            onSelectAudioInput(device)
            activeInputDeviceId = device.id
        },
    )

    Spacer(modifier = Modifier.height(16.dp))

    // Audio Output section
    SectionHeader(Strings.t("settings.incall.audioOutput", lang))
    AudioDeviceList(
        devices = outputDevices,
        lang = lang,
        testTagPrefix = "incall_audio_output",
        isActive = { device -> activeOutputDeviceId == device.id },
        onSelect = { device ->
            onSelectAudioOutput(device)
            activeOutputDeviceId = device.id
        },
    )
}

@Composable
private fun AudioDeviceList(
    devices: List<AudioDeviceInfo>,
    lang: String,
    testTagPrefix: String,
    isActive: (AudioDeviceInfo) -> Boolean,
    onSelect: (AudioDeviceInfo) -> Unit,
) {
    devices.forEachIndexed { index, device ->
        val label = audioDeviceLabel(device, lang)
        val active = isActive(device)
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("${testTagPrefix}_$index")
                    .clickable { onSelect(device) }
                    .padding(vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(
                selected = active,
                onClick = { onSelect(device) },
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
    val context = LocalContext.current
    val coroutineScope = rememberCoroutineScope()
    var selectedFront by remember { mutableStateOf(isFrontCamera) }

    // Background mode state: "off", "blur", or "image:<id>"
    var backgroundMode by remember {
        mutableStateOf(VisioManager.client.getBackgroundMode())
    }

    SectionHeader(Strings.t("settings.incall.cameraSelect", lang))

    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .testTag("incall_camera_front")
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
                .testTag("incall_camera_back")
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

    Spacer(modifier = Modifier.height(16.dp))

    // Background section
    SectionHeader(Strings.t("settings.incall.background", lang))

    // None / Blur row
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        BackgroundOptionChip(
            label = Strings.t("settings.incall.bgOff", lang),
            selected = backgroundMode == "off",
            onClick = {
                backgroundMode = "off"
                coroutineScope.launch(Dispatchers.IO) {
                    VisioManager.client.setBackgroundMode("off")
                }
            },
            modifier = Modifier.weight(1f),
        )
        BackgroundOptionChip(
            label = Strings.t("settings.incall.bgBlur", lang),
            selected = backgroundMode == "blur",
            onClick = {
                backgroundMode = "blur"
                coroutineScope.launch(Dispatchers.IO) {
                    VisioManager.client.setBackgroundMode("blur")
                }
            },
            modifier = Modifier.weight(1f),
        )
    }

    Spacer(modifier = Modifier.height(12.dp))

    // Image grid
    val imageIds = (1..8).toList()
    LazyVerticalGrid(
        columns = GridCells.Fixed(4),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.height(180.dp),
    ) {
        items(imageIds) { id ->
            val isSelected = backgroundMode == "image:$id"
            val bitmap =
                remember(id) {
                    BitmapFactory.decodeStream(
                        context.assets.open("backgrounds/thumbnails/$id.jpg"),
                    )
                }
            Box(
                modifier =
                    Modifier
                        .aspectRatio(1f)
                        .clip(RoundedCornerShape(8.dp))
                        .then(
                            if (isSelected) {
                                Modifier.border(
                                    2.dp,
                                    VisioColors.Primary500,
                                    RoundedCornerShape(8.dp),
                                )
                            } else {
                                Modifier
                            },
                        )
                        .clickable {
                            backgroundMode = "image:$id"
                            coroutineScope.launch(Dispatchers.IO) {
                                // Copy full image from assets to cache so FFI can read it by path
                                val cacheFile = java.io.File(context.cacheDir, "bg_$id.jpg")
                                if (!cacheFile.exists()) {
                                    context.assets.open("backgrounds/$id.jpg").use { input ->
                                        cacheFile.outputStream().use { output ->
                                            input.copyTo(output)
                                        }
                                    }
                                }
                                VisioManager.client.loadBackgroundImage(
                                    id.toUByte(),
                                    cacheFile.absolutePath,
                                )
                                VisioManager.client.setBackgroundMode("image:$id")
                            }
                        },
            ) {
                Image(
                    bitmap = bitmap.asImageBitmap(),
                    contentDescription = "Background $id",
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.matchParentSize(),
                )
            }
        }
    }
}

@Composable
private fun BackgroundOptionChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        contentAlignment = Alignment.Center,
        modifier =
            modifier
                .height(40.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(
                    if (selected) VisioColors.Primary500 else VisioColors.PrimaryDark100,
                )
                .clickable(onClick = onClick)
                .padding(horizontal = 12.dp),
    ) {
        Text(
            text = label,
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
private fun RoomInfoTab(
    roomUrl: String,
    lang: String,
) {
    val context = LocalContext.current
    val displayUrl = roomUrl.removePrefix("https://").removePrefix("http://")
    val deepLink = "visio://$displayUrl"
    var copiedHttp by remember { mutableStateOf(false) }
    var copiedDeep by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier.padding(8.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        // HTTPS link section
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.Language, contentDescription = null, tint = VisioColors.White, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(6.dp))
            Text(
                Strings.t("settings.incall.roomLink", lang),
                color = VisioColors.White.copy(alpha = 0.7f),
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = {
                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                clipboard.setPrimaryClip(android.content.ClipData.newPlainText("Room URL", roomUrl))
                copiedHttp = true
            }, modifier = Modifier.size(32.dp).testTag("incall_room_url_copy")) {
                Icon(
                    imageVector = if (copiedHttp) Icons.Outlined.Check else Icons.Outlined.ContentCopy,
                    contentDescription = if (copiedHttp) Strings.t("settings.incall.copied", lang) else Strings.t("info.copy", lang),
                    tint = VisioColors.White,
                    modifier = Modifier.size(16.dp),
                )
            }
            IconButton(onClick = {
                val shareIntent =
                    android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                        type = "text/plain"
                        putExtra(android.content.Intent.EXTRA_TEXT, roomUrl)
                    }
                context.startActivity(android.content.Intent.createChooser(shareIntent, null))
            }, modifier = Modifier.size(32.dp)) {
                Icon(
                    imageVector = Icons.Outlined.Share,
                    contentDescription = null,
                    tint = VisioColors.White,
                    modifier = Modifier.size(16.dp),
                )
            }
        }
        OutlinedTextField(
            value = roomUrl,
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            textStyle = MaterialTheme.typography.bodySmall.copy(color = VisioColors.White),
            modifier = Modifier.fillMaxWidth(),
            colors =
                OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = VisioColors.Primary500,
                    unfocusedBorderColor = VisioColors.White.copy(alpha = 0.3f),
                    cursorColor = VisioColors.Primary500,
                ),
        )

        Spacer(Modifier.height(8.dp))

        // Deep link section
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.PhoneAndroid, contentDescription = null, tint = VisioColors.White, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(6.dp))
            Text(
                Strings.t("settings.incall.deepLink", lang),
                color = VisioColors.White.copy(alpha = 0.7f),
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = {
                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                clipboard.setPrimaryClip(android.content.ClipData.newPlainText("Deep Link", deepLink))
                copiedDeep = true
            }, modifier = Modifier.size(32.dp)) {
                Icon(
                    imageVector = if (copiedDeep) Icons.Outlined.Check else Icons.Outlined.ContentCopy,
                    contentDescription = if (copiedDeep) Strings.t("settings.incall.copied", lang) else Strings.t("info.copy", lang),
                    tint = VisioColors.White,
                    modifier = Modifier.size(16.dp),
                )
            }
            IconButton(onClick = {
                val shareIntent =
                    android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                        type = "text/plain"
                        putExtra(android.content.Intent.EXTRA_TEXT, roomUrl)
                    }
                context.startActivity(android.content.Intent.createChooser(shareIntent, null))
            }, modifier = Modifier.size(32.dp)) {
                Icon(
                    imageVector = Icons.Outlined.Share,
                    contentDescription = null,
                    tint = VisioColors.White,
                    modifier = Modifier.size(16.dp),
                )
            }
        }
        OutlinedTextField(
            value = deepLink,
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            textStyle = MaterialTheme.typography.bodySmall.copy(color = VisioColors.White),
            modifier = Modifier.fillMaxWidth(),
            colors =
                OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = VisioColors.Primary500,
                    unfocusedBorderColor = VisioColors.White.copy(alpha = 0.3f),
                    cursorColor = VisioColors.Primary500,
                ),
        )
    }
}

@Composable
private fun MembersTab(lang: String) {
    val accesses by VisioManager.roomAccesses.collectAsState()
    var searchQuery by remember { mutableStateOf("") }
    var searchResults by remember { mutableStateOf<List<UserSearchResult>>(emptyList()) }

    LaunchedEffect(Unit) { VisioManager.refreshAccesses() }

    LaunchedEffect(searchQuery) {
        if (searchQuery.length < 3) {
            searchResults = emptyList()
            return@LaunchedEffect
        }
        delay(300)
        try {
            searchResults = VisioManager.client.searchUsers(searchQuery)
        } catch (_: Exception) {
            searchResults = emptyList()
        }
    }

    Column(
        modifier =
            Modifier
                .padding(8.dp)
                .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        SectionHeader(Strings.t("restricted.members", lang))

        // Search field
        OutlinedTextField(
            value = searchQuery,
            onValueChange = { searchQuery = it },
            placeholder = { Text(Strings.t("restricted.searchUsers", lang), color = VisioColors.White.copy(alpha = 0.5f)) },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            textStyle = MaterialTheme.typography.bodySmall.copy(color = VisioColors.White),
            colors =
                OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = VisioColors.Primary500,
                    unfocusedBorderColor = VisioColors.White.copy(alpha = 0.3f),
                    cursorColor = VisioColors.Primary500,
                ),
        )

        // Search results
        searchResults.forEach { user ->
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .clickable {
                            VisioManager.addAccessMember(user.id) {
                                searchQuery = ""
                                searchResults = emptyList()
                            }
                        }
                        .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        user.fullName ?: user.email,
                        color = VisioColors.White,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Text(
                        user.email,
                        color = VisioColors.White.copy(alpha = 0.6f),
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        // Current members
        accesses.forEach { access ->
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        access.user.fullName ?: access.user.email,
                        color = VisioColors.White,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Text(
                        Strings.t("restricted.${access.role}", lang),
                        color = VisioColors.White.copy(alpha = 0.6f),
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                if (access.role == "member") {
                    Button(
                        onClick = { VisioManager.removeAccessMember(access.id) },
                        colors =
                            ButtonDefaults.buttonColors(
                                containerColor = VisioColors.Error500.copy(alpha = 0.2f),
                                contentColor = VisioColors.Error500,
                            ),
                        modifier = Modifier.height(32.dp),
                        contentPadding = PaddingValues(horizontal = 12.dp),
                    ) {
                        Text(
                            Strings.t("restricted.remove", lang),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
        }
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
                val name = device.productName.toString()
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
        device.productName.toString().ifBlank { null }
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
