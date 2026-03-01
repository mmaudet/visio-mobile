package io.visio.mobile.ui

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import io.visio.mobile.R
import io.visio.mobile.VideoSurfaceView
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.visio.ConnectionState
import uniffi.visio.ParticipantInfo
import kotlin.math.absoluteValue

fun Context.findActivity(): Activity? {
    var ctx = this
    while (ctx is android.content.ContextWrapper) {
        if (ctx is Activity) return ctx
        ctx = ctx.baseContext
    }
    return null
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CallScreen(
    roomUrl: String,
    username: String,
    onNavigateToChat: () -> Unit,
    onHangUp: () -> Unit
) {
    val connectionState by VisioManager.connectionState.collectAsState()
    val participants by VisioManager.participants.collectAsState()
    val activeSpeakers by VisioManager.activeSpeakers.collectAsState()
    val handRaisedMap by VisioManager.handRaisedMap.collectAsState()
    val unreadCount by VisioManager.unreadCount.collectAsState()
    val isHandRaised by VisioManager.isHandRaised.collectAsState()

    val context = LocalContext.current
    val lang = VisioManager.currentLang
    var micEnabled by remember { mutableStateOf(true) }
    var cameraEnabled by remember { mutableStateOf(true) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var showAudioSheet by remember { mutableStateOf(false) }
    var showParticipantList by remember { mutableStateOf(false) }
    var focusedParticipantSid by remember { mutableStateOf<String?>(null) }

    val coroutineScope = rememberCoroutineScope()

    // Check if in PiP mode
    val isInPiP = context.findActivity()?.let {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) it.isInPictureInPictureMode else false
    } ?: false

    // Mic permission launcher
    val micPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            coroutineScope.launch(Dispatchers.IO) {
                try {
                    VisioManager.client.setMicrophoneEnabled(true)
                    VisioManager.startAudioCapture()
                    micEnabled = true
                } catch (_: Exception) {}
            }
        }
    }

    // Camera permission launcher
    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            coroutineScope.launch(Dispatchers.IO) {
                try {
                    VisioManager.client.setCameraEnabled(true)
                    VisioManager.startCameraCapture()
                    cameraEnabled = true
                } catch (_: Exception) {}
            }
        }
    }

    // Stop capture and playout when leaving the call screen
    DisposableEffect(Unit) {
        onDispose {
            VisioManager.stopCameraCapture()
            VisioManager.stopAudioCapture()
            VisioManager.stopAudioPlayout()
        }
    }

    // Connect on first composition
    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            try {
                val state = VisioManager.connectionState.value
                if (state is ConnectionState.Connected || state is ConnectionState.Connecting) {
                    micEnabled = VisioManager.client.isMicrophoneEnabled()
                    cameraEnabled = VisioManager.client.isCameraEnabled()
                    return@withContext
                }
                val user = username.ifBlank { null }
                VisioManager.client.connect(roomUrl, user)
                VisioManager.startAudioPlayout()
                micEnabled = VisioManager.client.isMicrophoneEnabled()
                cameraEnabled = VisioManager.client.isCameraEnabled()
            } catch (e: Exception) {
                errorMessage = e.message ?: "Connection failed"
            }
        }
    }

    // Notify backend when navigating to chat
    val onChatOpen = {
        coroutineScope.launch(Dispatchers.IO) {
            try { VisioManager.client.setChatOpen(true) } catch (_: Exception) {}
        }
        onNavigateToChat()
    }

    // PiP mode: show only active speaker, no controls
    if (isInPiP) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(VisioColors.PrimaryDark50),
            contentAlignment = Alignment.Center
        ) {
            val activeSpeakerSid = activeSpeakers.firstOrNull()
            val speaker = participants.find { it.sid == activeSpeakerSid } ?: participants.firstOrNull()
            if (speaker != null) {
                ParticipantTile(
                    participant = speaker,
                    isActiveSpeaker = false,
                    handRaisePosition = 0,
                    onClick = {}
                )
            }
        }
        return
    }

    // Participant list bottom sheet
    if (showParticipantList) {
        ParticipantListSheet(
            participants = participants,
            localDisplayName = username,
            localMicEnabled = micEnabled,
            localCameraEnabled = cameraEnabled,
            localIsHandRaised = isHandRaised,
            handRaisedMap = handRaisedMap,
            lang = lang,
            onDismiss = { showParticipantList = false }
        )
    }

    // Audio device bottom sheet
    if (showAudioSheet) {
        AudioDeviceSheet(
            onDismiss = { showAudioSheet = false },
            onSelect = { device ->
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
                    audioManager.setCommunicationDevice(device)
                }
                showAudioSheet = false
            }
        )
    }

    // Main call layout
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(VisioColors.PrimaryDark50)
    ) {
        Column(modifier = Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
            // Connection state banner
            ConnectionStateBanner(connectionState, errorMessage)

            // Video grid area
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(8.dp)
            ) {
                val focusedP = focusedParticipantSid?.let { sid -> participants.find { it.sid == sid } }

                if (focusedP != null) {
                    // Focus mode
                    Column(modifier = Modifier.fillMaxSize()) {
                        // Main focused participant
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                        ) {
                            ParticipantTile(
                                participant = focusedP,
                                isActiveSpeaker = activeSpeakers.contains(focusedP.sid),
                                handRaisePosition = handRaisedMap[focusedP.sid] ?: 0,
                                onClick = { focusedParticipantSid = null }
                            )
                        }

                        Spacer(modifier = Modifier.height(8.dp))

                        // Bottom strip of other participants
                        LazyRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.height(100.dp)
                        ) {
                            val others = participants.filter { it.sid != focusedP.sid }
                            items(others, key = { it.sid }) { p ->
                                Box(
                                    modifier = Modifier
                                        .width(140.dp)
                                        .height(100.dp)
                                        .clip(RoundedCornerShape(8.dp))
                                ) {
                                    ParticipantTile(
                                        participant = p,
                                        isActiveSpeaker = activeSpeakers.contains(p.sid),
                                        handRaisePosition = handRaisedMap[p.sid] ?: 0,
                                        onClick = { focusedParticipantSid = p.sid }
                                    )
                                }
                            }
                        }
                    }
                } else {
                    // Grid mode
                    val columns = when {
                        participants.size <= 1 -> 1
                        participants.size <= 4 -> 2
                        else -> 2
                    }

                    LazyVerticalGrid(
                        columns = GridCells.Fixed(columns),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.fillMaxSize()
                    ) {
                        items(participants, key = { it.sid }) { p ->
                            Box(
                                modifier = Modifier
                                    .aspectRatio(16f / 9f)
                                    .clip(RoundedCornerShape(8.dp))
                            ) {
                                ParticipantTile(
                                    participant = p,
                                    isActiveSpeaker = activeSpeakers.contains(p.sid),
                                    handRaisePosition = handRaisedMap[p.sid] ?: 0,
                                    onClick = { focusedParticipantSid = p.sid }
                                )
                            }
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Control bar
            ControlBar(
                micEnabled = micEnabled,
                cameraEnabled = cameraEnabled,
                isHandRaised = isHandRaised,
                unreadCount = unreadCount,
                participantCount = participants.size + 1,
                lang = lang,
                onToggleMic = {
                    val newState = !micEnabled
                    if (newState) {
                        val hasPermission = ContextCompat.checkSelfPermission(
                            context, Manifest.permission.RECORD_AUDIO
                        ) == PackageManager.PERMISSION_GRANTED
                        if (hasPermission) {
                            coroutineScope.launch(Dispatchers.IO) {
                                try {
                                    VisioManager.client.setMicrophoneEnabled(true)
                                    VisioManager.startAudioCapture()
                                    micEnabled = true
                                } catch (_: Exception) {}
                            }
                        } else {
                            micPermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
                        }
                    } else {
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                VisioManager.stopAudioCapture()
                                VisioManager.client.setMicrophoneEnabled(false)
                                micEnabled = false
                            } catch (_: Exception) {}
                        }
                    }
                },
                onAudioPicker = { showAudioSheet = true },
                onToggleCamera = {
                    val newState = !cameraEnabled
                    if (newState) {
                        val hasPermission = ContextCompat.checkSelfPermission(
                            context, Manifest.permission.CAMERA
                        ) == PackageManager.PERMISSION_GRANTED
                        if (hasPermission) {
                            coroutineScope.launch(Dispatchers.IO) {
                                try {
                                    VisioManager.client.setCameraEnabled(true)
                                    VisioManager.startCameraCapture()
                                    cameraEnabled = true
                                    VisioManager.refreshParticipantsPublic()
                                } catch (_: Exception) {}
                            }
                        } else {
                            cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
                        }
                    } else {
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                VisioManager.stopCameraCapture()
                                VisioManager.client.setCameraEnabled(false)
                                cameraEnabled = false
                                VisioManager.refreshParticipantsPublic()
                            } catch (_: Exception) {}
                        }
                    }
                },
                onSwitchCamera = {
                    // Camera switch (front/back) - placeholder
                },
                onToggleHandRaise = {
                    coroutineScope.launch(Dispatchers.IO) {
                        try {
                            if (isHandRaised) {
                                VisioManager.client.lowerHand()
                            } else {
                                VisioManager.client.raiseHand()
                            }
                        } catch (_: Exception) {}
                    }
                },
                onParticipants = { showParticipantList = true },
                onChat = onChatOpen,
                onHangUp = {
                    VisioManager.stopCameraCapture()
                    VisioManager.stopAudioCapture()
                    VisioManager.stopAudioPlayout()
                    VisioManager.client.disconnect()
                    onHangUp()
                }
            )

            Spacer(modifier = Modifier.height(8.dp))
        }
    }
}

@Composable
private fun ControlBar(
    micEnabled: Boolean,
    cameraEnabled: Boolean,
    isHandRaised: Boolean,
    unreadCount: Int,
    participantCount: Int,
    lang: String,
    onToggleMic: () -> Unit,
    onAudioPicker: () -> Unit,
    onToggleCamera: () -> Unit,
    onSwitchCamera: () -> Unit,
    onToggleHandRaise: () -> Unit,
    onParticipants: () -> Unit,
    onChat: () -> Unit,
    onHangUp: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .background(VisioColors.PrimaryDark75, RoundedCornerShape(16.dp))
            .padding(12.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Mic group: toggle + audio picker chevron
        Row(
            modifier = Modifier
                .background(
                    if (micEnabled) VisioColors.PrimaryDark100 else VisioColors.Error200,
                    RoundedCornerShape(8.dp)
                ),
            verticalAlignment = Alignment.CenterVertically
        ) {
            IconButton(
                onClick = onToggleMic,
                modifier = Modifier.size(44.dp)
            ) {
                Icon(
                    painter = painterResource(
                        if (micEnabled) R.drawable.ri_mic_line else R.drawable.ri_mic_off_line
                    ),
                    contentDescription = if (micEnabled) Strings.t("control.mute", lang) else Strings.t("control.unmute", lang),
                    tint = VisioColors.White,
                    modifier = Modifier.size(20.dp)
                )
            }
            IconButton(
                onClick = onAudioPicker,
                modifier = Modifier.size(28.dp, 44.dp)
            ) {
                Icon(
                    painter = painterResource(R.drawable.ri_arrow_up_s_line),
                    contentDescription = Strings.t("control.audioDevices", lang),
                    tint = VisioColors.White,
                    modifier = Modifier.size(16.dp)
                )
            }
        }

        // Camera toggle
        IconButton(
            onClick = onToggleCamera,
            modifier = Modifier
                .size(44.dp)
                .background(
                    if (cameraEnabled) VisioColors.PrimaryDark100 else VisioColors.Error200,
                    RoundedCornerShape(8.dp)
                )
        ) {
            Icon(
                painter = painterResource(
                    if (cameraEnabled) R.drawable.ri_video_on_line else R.drawable.ri_video_off_line
                ),
                contentDescription = if (cameraEnabled) Strings.t("control.camOff", lang) else Strings.t("control.camOn", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(20.dp)
            )
        }

        // Camera switch (front/back)
        IconButton(
            onClick = onSwitchCamera,
            modifier = Modifier
                .size(44.dp)
                .background(VisioColors.PrimaryDark100, RoundedCornerShape(8.dp))
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_camera_switch_line),
                contentDescription = Strings.t("call.switchCamera", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(20.dp)
            )
        }

        // Hand raise
        IconButton(
            onClick = onToggleHandRaise,
            modifier = Modifier
                .size(44.dp)
                .background(
                    if (isHandRaised) VisioColors.HandRaise else VisioColors.PrimaryDark100,
                    RoundedCornerShape(8.dp)
                )
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_hand),
                contentDescription = if (isHandRaised) Strings.t("control.lowerHand", lang) else Strings.t("control.raiseHand", lang),
                tint = if (isHandRaised) Color.Black else VisioColors.White,
                modifier = Modifier.size(20.dp)
            )
        }

        // Participants with count badge
        IconButton(
            onClick = onParticipants,
            modifier = Modifier
                .size(44.dp)
                .background(VisioColors.PrimaryDark100, RoundedCornerShape(8.dp))
        ) {
            BadgedBox(
                badge = {
                    if (participantCount > 0) {
                        Badge(
                            containerColor = VisioColors.Primary500,
                            contentColor = VisioColors.White
                        ) {
                            Text(
                                text = "$participantCount",
                                fontSize = 10.sp
                            )
                        }
                    }
                }
            ) {
                Icon(
                    painter = painterResource(R.drawable.ri_group_line),
                    contentDescription = Strings.t("participants.title", lang),
                    tint = VisioColors.White,
                    modifier = Modifier.size(20.dp)
                )
            }
        }

        // Chat with unread badge
        IconButton(
            onClick = onChat,
            modifier = Modifier
                .size(44.dp)
                .background(VisioColors.PrimaryDark100, RoundedCornerShape(8.dp))
        ) {
            BadgedBox(
                badge = {
                    if (unreadCount > 0) {
                        Badge(
                            containerColor = VisioColors.Error500,
                            contentColor = VisioColors.White
                        ) {
                            Text(
                                text = if (unreadCount > 9) "9+" else "$unreadCount",
                                fontSize = 10.sp
                            )
                        }
                    }
                }
            ) {
                Icon(
                    painter = painterResource(R.drawable.ri_chat_1_line),
                    contentDescription = Strings.t("chat", lang),
                    tint = VisioColors.White,
                    modifier = Modifier.size(20.dp)
                )
            }
        }

        // Hangup
        IconButton(
            onClick = onHangUp,
            modifier = Modifier
                .size(44.dp)
                .background(VisioColors.Error500, RoundedCornerShape(8.dp))
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_phone_fill),
                contentDescription = Strings.t("control.leave", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(20.dp)
            )
        }
    }
}

@Composable
fun ParticipantTile(
    participant: ParticipantInfo,
    isActiveSpeaker: Boolean,
    handRaisePosition: Int,
    onClick: () -> Unit
) {
    val lang = VisioManager.currentLang
    val name = participant.name ?: participant.identity
    val initials = name
        .split(" ")
        .mapNotNull { it.firstOrNull()?.uppercase() }
        .take(2)
        .joinToString("")
        .ifEmpty { "?" }

    // Deterministic hue from name
    val hue = name.fold(0) { acc, c -> acc + c.code }.absoluteValue % 360
    val avatarColor = Color.hsl(hue.toFloat(), 0.5f, 0.35f)

    val borderColor = if (isActiveSpeaker) VisioColors.Primary500 else Color.Transparent
    val borderMod = if (isActiveSpeaker) {
        Modifier
            .border(2.dp, borderColor, RoundedCornerShape(8.dp))
            .shadow(8.dp, RoundedCornerShape(8.dp), ambientColor = VisioColors.Primary500)
    } else {
        Modifier
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .then(borderMod)
            .clip(RoundedCornerShape(8.dp))
            .background(VisioColors.PrimaryDark50)
            .clickable(onClick = onClick)
    ) {
        // Video surface or avatar fallback
        if (participant.hasVideo && participant.videoTrackSid != null) {
            val trackSid = participant.videoTrackSid!!
            AndroidView(
                factory = { ctx -> VideoSurfaceView(ctx, trackSid) },
                modifier = Modifier.fillMaxSize()
            )
        } else {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center
            ) {
                Box(
                    modifier = Modifier
                        .size(64.dp)
                        .clip(CircleShape)
                        .background(avatarColor),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = initials,
                        color = VisioColors.White,
                        fontSize = 24.sp,
                        fontWeight = FontWeight.Bold
                    )
                }
            }
        }

        // Metadata bar at bottom
        Row(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .background(Color(0x99000000))
                .padding(horizontal = 8.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            // Mic muted indicator
            if (participant.isMuted) {
                Icon(
                    painter = painterResource(R.drawable.ri_mic_off_fill),
                    contentDescription = Strings.t("accessibility.muted", lang),
                    tint = VisioColors.Error500,
                    modifier = Modifier.size(14.dp)
                )
            }

            // Hand raise badge
            if (handRaisePosition > 0) {
                Row(
                    modifier = Modifier
                        .background(VisioColors.HandRaise, RoundedCornerShape(10.dp))
                        .padding(horizontal = 6.dp, vertical = 1.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(2.dp)
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ri_hand),
                        contentDescription = null,
                        tint = Color.Black,
                        modifier = Modifier.size(12.dp)
                    )
                    Text(
                        text = "$handRaisePosition",
                        color = Color.Black,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold
                    )
                }
            }

            // Name
            Text(
                text = name,
                color = VisioColors.White,
                fontSize = 12.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f)
            )

            // Connection quality bars
            ConnectionQualityBars(participant.connectionQuality.name)
        }
    }
}

@Composable
private fun ConnectionQualityBars(quality: String) {
    val bars = when (quality) {
        "Excellent" -> 3
        "Good" -> 2
        "Poor" -> 1
        else -> 0
    }
    Row(
        horizontalArrangement = Arrangement.spacedBy(1.dp),
        verticalAlignment = Alignment.Bottom
    ) {
        for (i in 1..3) {
            Box(
                modifier = Modifier
                    .width(3.dp)
                    .height((i * 4 + 2).dp)
                    .background(
                        if (i <= bars) Color.Green else VisioColors.Greyscale400,
                        RoundedCornerShape(1.dp)
                    )
            )
        }
    }
}

@Composable
private fun ConnectionStateBanner(state: ConnectionState, errorMessage: String?) {
    val lang = VisioManager.currentLang
    when {
        errorMessage != null -> {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(VisioColors.Error200)
                    .padding(12.dp)
            ) {
                Text(
                    text = "${Strings.t("call.error", lang)}: $errorMessage",
                    color = VisioColors.Error500,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }
        state is ConnectionState.Connecting -> {
            Row(
                modifier = Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    color = VisioColors.Primary500
                )
                Text(
                    "${Strings.t("status.connecting", lang)}...",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onBackground
                )
            }
        }
        state is ConnectionState.Reconnecting -> {
            Row(
                modifier = Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    color = VisioColors.Primary500
                )
                Text(
                    "${Strings.t("status.reconnecting", lang)} (${state.attempt})...",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onBackground
                )
            }
        }
        // Connected / Disconnected: no banner
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AudioDeviceSheet(
    onDismiss: () -> Unit,
    onSelect: (AudioDeviceInfo) -> Unit
) {
    val context = LocalContext.current
    val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    val devices = remember {
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

    val sheetState = rememberModalBottomSheetState()

    val lang = VisioManager.currentLang

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = VisioColors.PrimaryDark75
    ) {
        Text(
            text = Strings.t("audio.source", lang),
            style = MaterialTheme.typography.titleMedium,
            color = VisioColors.White,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
        )

        devices.forEach { device ->
            val label = device.productName?.toString()?.ifBlank { null }
                ?: audioDeviceTypeName(device.type, lang)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onSelect(device) }
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = label,
                    color = VisioColors.White,
                    style = MaterialTheme.typography.bodyLarge
                )
            }
        }

        Spacer(modifier = Modifier.height(32.dp))
    }
}

private fun audioDeviceTypeName(type: Int, lang: String): String = when (type) {
    AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> Strings.t("audio.speaker", lang)
    AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> Strings.t("audio.earpiece", lang)
    AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> Strings.t("audio.bluetooth", lang)
    AudioDeviceInfo.TYPE_BLUETOOTH_SCO -> Strings.t("audio.bluetooth", lang)
    AudioDeviceInfo.TYPE_WIRED_HEADSET -> Strings.t("audio.wiredHeadset", lang)
    AudioDeviceInfo.TYPE_WIRED_HEADPHONES -> Strings.t("audio.wiredHeadphones", lang)
    AudioDeviceInfo.TYPE_USB_HEADSET -> Strings.t("audio.usbHeadset", lang)
    else -> Strings.t("audio.device", lang)
}
