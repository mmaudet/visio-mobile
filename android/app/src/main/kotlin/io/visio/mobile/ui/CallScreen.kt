package io.visio.mobile.ui

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Fullscreen
import androidx.compose.material.icons.outlined.VideocamOff
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import io.visio.mobile.R
import io.visio.mobile.ReactionData
import io.visio.mobile.VideoSurfaceView
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.visio.AdaptiveMode
import uniffi.visio.BandwidthMode
import uniffi.visio.ConnectionState
import uniffi.visio.ParticipantInfo
import uniffi.visio.WaitingParticipant
import kotlin.math.absoluteValue

private const val TAG = "CallScreen"

data class FocusItem(val participantSid: String, val source: String)

data class DisplayItem(
    val key: String,
    val participant: ParticipantInfo,
    val source: String,
    val trackSid: String?,
    val isScreenShare: Boolean,
    val label: String,
)

fun buildDisplayItems(participants: List<ParticipantInfo>): List<DisplayItem> {
    val items = mutableListOf<DisplayItem>()
    for (p in participants) {
        val name = p.name ?: p.identity
        items.add(
            DisplayItem(
                key = "${p.sid}-camera",
                participant = p,
                source = "camera",
                trackSid = p.videoTrackSid,
                isScreenShare = false,
                label = name,
            ),
        )
        if (p.hasScreenShare && p.screenShareTrackSid != null) {
            items.add(
                DisplayItem(
                    key = "${p.sid}-screen",
                    participant = p,
                    source = "screen_share",
                    trackSid = p.screenShareTrackSid,
                    isScreenShare = true,
                    label = name,
                ),
            )
        }
    }
    return items
}

private val REACTION_EMOJIS =
    listOf(
        "thumbsUp" to "\uD83D\uDC4D",
        "clap" to "\uD83D\uDC4F",
        "joy" to "\uD83D\uDE02",
        "openMouth" to "\uD83D\uDE2E",
        "tada" to "\uD83C\uDF89",
        "heart" to "\u2764\uFE0F",
    )

fun Context.findActivity(): Activity? {
    var ctx = this
    while (ctx is android.content.ContextWrapper) {
        if (ctx is Activity) return ctx
        ctx = ctx.baseContext
    }
    return null
}

@Suppress("kotlin:S3776", "kotlin:S6615")
private suspend fun handleTestConnect(
    testConnect: Triple<String, String, String?>,
    coroutineScope: CoroutineScope,
    onError: (String) -> Unit,
    onMicState: (Boolean) -> Unit,
    onCameraState: (Boolean) -> Unit,
) {
    val (livekitUrl, token, mediaFile) = testConnect
    VisioManager.pendingTestConnect = null
    VisioManager.isTestConnection = true
    Log.i(TAG, "Test deep link: connecting directly to $livekitUrl, media=$mediaFile")
    try {
        VisioManager.client.connectWithToken(livekitUrl, token)
    } catch (e: Exception) {
        onError("Test connection failed: ${e.message}")
        return
    }

    try {
        VisioManager.startAudioPlayout()
    } catch (e: Exception) {
        onError("Audio playout failed: ${e.message}")
        return
    }

    val settings =
        try {
            VisioManager.client.getSettings()
        } catch (_: Exception) {
            null
        }
    onMicState(settings?.micEnabledOnJoin ?: true)
    onCameraState(true)

    try {
        VisioManager.startAudioCapture()
    } catch (_: Exception) {
        // No-op
    }

    val hasMediaFile = !mediaFile.isNullOrBlank() && java.io.File(mediaFile).exists()

    fun startMediaCapture() {
        if (hasMediaFile) {
            VisioManager.startMediaFileCapture(mediaFile!!)
        } else {
            VisioManager.startSyntheticAudioCapture()
            VisioManager.startCameraCapture()
        }
    }

    fun stopMediaCapture() {
        if (hasMediaFile) {
            VisioManager.stopMediaFileCapture()
        } else {
            VisioManager.stopAudioCapture()
            VisioManager.stopCameraCapture()
        }
    }

    launchTestChatMessages(coroutineScope)
    launchTestTurnSequence(coroutineScope, ::startMediaCapture, ::stopMediaCapture)
}

@Suppress("kotlin:S6615")
private fun launchTestChatMessages(coroutineScope: CoroutineScope) {
    coroutineScope.launch(Dispatchers.IO) {
        delay(3000)
        try {
            VisioManager.client.sendChatMessage("Android joined the room!")
        } catch (_: Exception) {
            // No-op
        }
        delay(47000)
        try {
            VisioManager.client.sendChatMessage("Android: my turn to speak!")
        } catch (_: Exception) {
            // No-op
        }
        delay(15000)
        try {
            VisioManager.client.sendChatMessage("Android: mid-turn check-in")
        } catch (_: Exception) {
            // No-op
        }
        delay(10000)
        try {
            VisioManager.client.sendChatMessage("Android: muted — iOS's turn")
        } catch (_: Exception) {
            // No-op
        }
        delay(25000)
        try {
            VisioManager.client.sendChatMessage("Android: everyone speaking together!")
        } catch (_: Exception) {
            // No-op
        }
    }
}

@Suppress("kotlin:S3776", "kotlin:S6615")
private fun launchTestTurnSequence(
    coroutineScope: CoroutineScope,
    startMediaCapture: () -> Unit,
    stopMediaCapture: () -> Unit,
) {
    coroutineScope.launch(Dispatchers.IO) {
        delay(5000)
        Log.i(TAG, "[TURN] Android muted (bot's turn)")
        try {
            stopMediaCapture()
            VisioManager.client.setMicrophoneEnabled(false)
        } catch (_: Exception) {
            // No-op
        }
        try {
            VisioManager.client.setCameraEnabled(false)
        } catch (_: Exception) {
            // No-op
        }

        delay(45000)
        Log.i(TAG, "[TURN] Android speaking")
        try {
            VisioManager.client.setMicrophoneEnabled(true)
            VisioManager.client.setCameraEnabled(true)
        } catch (_: Exception) {
            // No-op
        }
        try {
            startMediaCapture()
        } catch (_: Exception) {
            // No-op
        }

        delay(25000)
        Log.i(TAG, "[TURN] Android muted (iOS's turn)")
        try {
            stopMediaCapture()
            VisioManager.client.setMicrophoneEnabled(false)
        } catch (_: Exception) {
            // No-op
        }
        try {
            VisioManager.client.setCameraEnabled(false)
        } catch (_: Exception) {
            // No-op
        }

        delay(25000)
        Log.i(TAG, "[TURN] Android unmuted (all speak)")
        try {
            VisioManager.client.setMicrophoneEnabled(true)
            VisioManager.client.setCameraEnabled(true)
        } catch (_: Exception) {
            // No-op
        }
        try {
            startMediaCapture()
        } catch (_: Exception) {
            // No-op
        }
    }
}

private suspend fun handleNormalConnect(
    context: Context,
    roomUrl: String,
    username: String,
    onError: (String) -> Unit,
    onMicState: (Boolean) -> Unit,
    onCameraState: (Boolean) -> Unit,
) {
    val settings =
        try {
            VisioManager.client.getSettings()
        } catch (e: Exception) {
            onError("Failed to load settings: ${e.message}")
            return
        }

    val user = username.ifBlank { null }
    try {
        VisioManager.client.connect(roomUrl, user)
    } catch (e: Exception) {
        onError("Connection failed: ${e.message}")
        return
    }

    try {
        VisioManager.startAudioPlayout()
    } catch (e: Exception) {
        onError("Audio playout failed: ${e.message}")
        return
    }

    // Apply audio device preferences saved from lobby (if any)
    VisioManager.pendingOutputDevice?.let { device ->
        try {
            VisioManager.setAudioOutputDevice(device)
        } catch (_: Exception) {
            // No-op
        }
    }
    VisioManager.pendingOutputDevice = null

    applyMicOnJoin(context, settings.micEnabledOnJoin)
    onMicState(VisioManager.client.isMicrophoneEnabled())

    applyCameraOnJoin(context, settings.cameraEnabledOnJoin)
    onCameraState(VisioManager.client.isCameraEnabled())
}

private fun applyMicOnJoin(
    context: Context,
    micEnabledOnJoin: Boolean,
) {
    if (!micEnabledOnJoin) {
        VisioManager.pendingInputDevice = null
        return
    }
    val hasMicPerm =
        ContextCompat.checkSelfPermission(
            context, Manifest.permission.RECORD_AUDIO,
        ) == PackageManager.PERMISSION_GRANTED
    if (hasMicPerm) {
        try {
            VisioManager.client.setMicrophoneEnabled(true)
            VisioManager.startAudioCapture(VisioManager.pendingInputDevice)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to enable microphone on join", e)
        }
    }
    VisioManager.pendingInputDevice = null
}

private fun applyCameraOnJoin(
    context: Context,
    cameraEnabledOnJoin: Boolean,
) {
    if (!cameraEnabledOnJoin) return
    val hasCamPerm =
        ContextCompat.checkSelfPermission(
            context, Manifest.permission.CAMERA,
        ) == PackageManager.PERMISSION_GRANTED
    if (hasCamPerm) {
        try {
            VisioManager.client.setCameraEnabled(true)
            VisioManager.startCameraCapture()
            VisioManager.refreshParticipantsPublic()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to enable camera on join", e)
        }
    }
}

private fun toggleMic(
    micEnabled: Boolean,
    context: Context,
    coroutineScope: CoroutineScope,
    micPermissionLauncher: androidx.activity.result.ActivityResultLauncher<String>,
    onMicState: (Boolean) -> Unit,
) {
    val newState = !micEnabled
    if (newState) {
        val hasPermission =
            ContextCompat.checkSelfPermission(
                context, Manifest.permission.RECORD_AUDIO,
            ) == PackageManager.PERMISSION_GRANTED
        if (hasPermission) {
            coroutineScope.launch(Dispatchers.IO) {
                try {
                    VisioManager.client.setMicrophoneEnabled(true)
                    VisioManager.startAudioCapture()
                    onMicState(true)
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to enable microphone", e)
                }
            }
        } else {
            micPermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
        }
    } else {
        coroutineScope.launch(Dispatchers.IO) {
            try {
                VisioManager.stopAudioCapture()
                VisioManager.client.setMicrophoneEnabled(false)
                onMicState(false)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to disable microphone", e)
            }
        }
    }
}

private fun toggleCamera(
    cameraEnabled: Boolean,
    context: Context,
    coroutineScope: CoroutineScope,
    cameraPermissionLauncher: androidx.activity.result.ActivityResultLauncher<String>,
    onCameraState: (Boolean) -> Unit,
) {
    val newState = !cameraEnabled
    if (newState) {
        val hasPermission =
            ContextCompat.checkSelfPermission(
                context, Manifest.permission.CAMERA,
            ) == PackageManager.PERMISSION_GRANTED
        if (hasPermission) {
            coroutineScope.launch(Dispatchers.IO) {
                try {
                    VisioManager.startCameraCapture()
                    VisioManager.client.setCameraEnabled(true)
                    onCameraState(true)
                    VisioManager.refreshParticipantsPublic()
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to enable camera", e)
                }
            }
        } else {
            cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
        }
    } else {
        coroutineScope.launch(Dispatchers.IO) {
            try {
                VisioManager.stopCameraCapture()
                VisioManager.client.setCameraEnabled(false)
                onCameraState(false)
                VisioManager.refreshParticipantsPublic()
            } catch (e: Exception) {
                Log.e(TAG, "Failed to disable camera", e)
            }
        }
    }
}

@Suppress("kotlin:S3776", "kotlin:S6615")
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CallScreen(
    roomUrl: String,
    username: String,
    roomDisplayName: String? = null,
    onNavigateToChat: () -> Unit,
    onHangUp: () -> Unit,
) {
    val connectionState by VisioManager.connectionState.collectAsState()
    val participants by VisioManager.participants.collectAsState()
    val activeSpeakers by VisioManager.activeSpeakers.collectAsState()
    val handRaisedMap by VisioManager.handRaisedMap.collectAsState()
    val unreadCount by VisioManager.unreadCount.collectAsState()
    val isHandRaised by VisioManager.isHandRaised.collectAsState()
    val lobbyNotification by VisioManager.lobbyNotification.collectAsState()
    val waitingParticipants by VisioManager.waitingParticipants.collectAsState()
    val adaptiveMode by VisioManager.adaptiveMode.collectAsState()
    val bandwidthMode by VisioManager.bandwidthMode.collectAsState()
    val isAdaptiveModeEnabled =
        try {
            VisioManager.client.isAdaptiveModeEnabled()
        } catch (_: Exception) {
            false
        }

    val context = LocalContext.current
    val lang = VisioManager.currentLang
    var micEnabled by remember { mutableStateOf(false) }
    var cameraEnabled by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var showInCallSettings by remember { mutableStateOf(false) }
    var inCallSettingsTab by remember { mutableIntStateOf(0) }
    var showParticipantList by remember { mutableStateOf(false) }
    var focusedItem by remember { mutableStateOf<FocusItem?>(null) }
    // Track whether focus was set by user (pin) vs auto (active speaker / screen share)
    var userPinnedItem by remember { mutableStateOf<FocusItem?>(null) }
    var layoutState by remember { mutableStateOf(LayoutState()) }
    var showReactionPicker by remember { mutableStateOf(false) }
    // Fullscreen controls visibility: toggles on tap, auto-hides after 3s
    var controlsVisible by remember { mutableStateOf(false) }
    val reactions by VisioManager.reactions.collectAsState()
    val screenShareSubscribed by VisioManager.screenShareSubscribed.collectAsState()

    // Auto-hide controls after 3 seconds in focus mode; reset when leaving focus
    LaunchedEffect(controlsVisible, focusedItem) {
        if (focusedItem == null) {
            controlsVisible = false
        } else if (controlsVisible) {
            delay(3000)
            controlsVisible = false
        }
    }

    // Auto-focus on screen share arrival
    LaunchedEffect(screenShareSubscribed) {
        val sid = screenShareSubscribed
        if (sid != null) {
            focusedItem = FocusItem(sid, "screen_share")
            VisioManager.clearScreenShareSubscribed()
        }
    }

    // Auto-unfocus when screen share ends
    LaunchedEffect(participants, focusedItem) {
        if (focusedItem?.source == "screen_share") {
            val p = participants.find { it.sid == focusedItem?.participantSid }
            @Suppress("kotlin:S1125")
            if (p?.hasScreenShare != true) {
                focusedItem = null
            }
        }
    }

    // When adaptive mode is disabled, always use Office layout
    val effectiveAdaptiveMode = if (isAdaptiveModeEnabled) adaptiveMode else AdaptiveMode.OFFICE

    // Layout engine: replaces old active speaker auto-focus with unified layout computation
    val screenShareFocus = screenShareSubscribed?.let { FocusItem(it, "screen_share") }

    LaunchedEffect(participants, activeSpeakers, userPinnedItem, effectiveAdaptiveMode, screenShareFocus) {
        val localSid = participants.firstOrNull()?.sid ?: return@LaunchedEffect
        val (decision, newState) =
            computeLayout(
                participants = participants,
                activeSpeakers = activeSpeakers,
                pinnedItem = userPinnedItem,
                screenShare = screenShareFocus,
                adaptiveMode = effectiveAdaptiveMode,
                localParticipantSid = localSid,
                previousState = layoutState,
                nowMs = System.currentTimeMillis(),
            )
        layoutState = newState
        // Update focusedItem based on decision
        when (decision.mode) {
            LayoutMode.GRID -> focusedItem = null
            LayoutMode.FOCUS -> {
                val main = decision.mainTile
                if (main != null) {
                    focusedItem = FocusItem(main.participant.sid, main.source)
                }
            }
        }
    }

    var lastMode by remember { mutableStateOf(adaptiveMode) }

    LaunchedEffect(adaptiveMode) {
        if (adaptiveMode != lastMode) {
            lastMode = adaptiveMode
            // Sync local cameraEnabled with actual Rust state (FFI call off main thread)
            val camState = withContext(Dispatchers.IO) { VisioManager.client.isCameraEnabled() }
            cameraEnabled = camState
        }
    }

    val coroutineScope = rememberCoroutineScope()

    // Check if in PiP mode
    val isInPiP =
        context.findActivity()?.let {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) it.isInPictureInPictureMode else false
        } ?: false

    // Mic permission launcher
    val micPermissionLauncher =
        rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted ->
            if (granted) {
                coroutineScope.launch(Dispatchers.IO) {
                    try {
                        VisioManager.client.setMicrophoneEnabled(true)
                        VisioManager.startAudioCapture()
                        micEnabled = true
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed to enable microphone after permission grant", e)
                    }
                }
            }
        }

    // Camera permission launcher
    val cameraPermissionLauncher =
        rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted ->
            if (granted) {
                coroutineScope.launch(Dispatchers.IO) {
                    try {
                        VisioManager.client.setCameraEnabled(true)
                        VisioManager.startCameraCapture()
                        cameraEnabled = true
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed to enable camera after permission grant", e)
                    }
                }
            }
        }

    // Keep screen on while connected or reconnecting
    val keepScreenOn =
        connectionState is ConnectionState.Connected ||
            connectionState is ConnectionState.Reconnecting
    DisposableEffect(keepScreenOn) {
        val window = context.findActivity()?.window
        if (keepScreenOn) {
            window?.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        } else {
            window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
        onDispose {
            window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
    }

    // Handle lobby denied — show toast and navigate back
    val lobbyDenied by VisioManager.lobbyDenied.collectAsState()
    LaunchedEffect(lobbyDenied) {
        if (lobbyDenied) {
            Toast.makeText(context, Strings.t("lobby.denied", lang), Toast.LENGTH_LONG).show()
            VisioManager.lobbyDenied.value = false
            VisioManager.disconnect()
            onHangUp()
        }
    }

    // Debug: log every composition
    Log.d(TAG, "CallScreen composed, connectionState=$connectionState, cam=$cameraEnabled")

    // WaitingForHost: show waiting screen instead of call UI
    if (connectionState is ConnectionState.WaitingForHost) {
        WaitingScreen(
            onCancel = {
                VisioManager.cancelLobby()
                VisioManager.disconnect()
                onHangUp()
            },
        )
        return
    }

    // Initialize call state. When coming from PreJoinScreen, media is already
    // enabled — just read the current state. For test deep links, do full setup.
    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            val state = VisioManager.connectionState.value
            Log.i(TAG, "CallScreen init: connectionState=$state")
            if (state is ConnectionState.Connected || state is ConnectionState.Connecting) {
                // PreJoinScreen already started audio/mic/camera — just sync UI state
                micEnabled = VisioManager.client.isMicrophoneEnabled()
                cameraEnabled = VisioManager.client.isCameraEnabled()
                Log.i(TAG, "CallScreen: inherited mic=$micEnabled, cam=$cameraEnabled")
                return@withContext
            }

            val testConnect = VisioManager.pendingTestConnect
            if (testConnect != null) {
                handleTestConnect(
                    testConnect,
                    coroutineScope,
                    onError = { errorMessage = it },
                    onMicState = { micEnabled = it },
                    onCameraState = { cameraEnabled = it },
                )
                return@withContext
            }

            handleNormalConnect(
                context,
                roomUrl,
                username,
                onError = { errorMessage = it },
                onMicState = { micEnabled = it },
                onCameraState = { cameraEnabled = it },
            )
        }
    }

    // BLUETOOTH_CONNECT permission is now requested in PreJoinScreen
    // so it doesn't interrupt the active call with a system dialog.

    // Notify backend when navigating to chat
    val onChatOpen = {
        coroutineScope.launch(Dispatchers.IO) {
            try {
                VisioManager.client.setChatOpen(true)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to notify backend that chat is open", e)
            }
        }
        onNavigateToChat()
    }

    // PiP mode: show only active speaker, no controls
    if (isInPiP) {
        CallPiPView(
            participants = participants,
            activeSpeakers = activeSpeakers,
        )
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
            onDismiss = { showParticipantList = false },
        )
    }

    // In-call settings bottom sheet (replaces audio device sheet)
    if (showInCallSettings) {
        InCallSettingsSheet(
            roomUrl = roomUrl,
            roomName = roomDisplayName,
            initialTab = inCallSettingsTab,
            onDismiss = { showInCallSettings = false },
            onSelectAudioInput = { device -> VisioManager.setAudioInputDevice(device) },
            onSelectAudioOutput = { device -> VisioManager.setAudioOutputDevice(device) },
            onSwitchCamera = { useFront ->
                VisioManager.switchCamera(useFront)
            },
            isFrontCamera = VisioManager.isFrontCamera(),
        )
    }

    // Main call layout
    val callBackground = if (effectiveAdaptiveMode == AdaptiveMode.OFFICE) VisioColors.PrimaryDark50 else Color.Black
    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .background(callBackground)
                .testTag("adaptive-mode:${effectiveAdaptiveMode.name}"),
    ) {
        Column(modifier = Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
            // Connection state banner
            ConnectionStateBanner(connectionState, errorMessage)

            // Bandwidth degradation banner
            BandwidthBanner(bandwidthMode)

            // Room display name header
            if (roomDisplayName != null) {
                Text(
                    text = roomDisplayName,
                    style = MaterialTheme.typography.labelMedium,
                    color = VisioColors.White.copy(alpha = 0.7f),
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                )
            }

            // Compute layout decision for rendering
            val localSid = participants.firstOrNull()?.sid ?: ""
            val renderScreenShareFocus = if (screenShareSubscribed != null) FocusItem(screenShareSubscribed!!, "screen_share") else null
            val (layoutDecision, _) =
                computeLayout(
                    participants = participants,
                    activeSpeakers = activeSpeakers,
                    pinnedItem = userPinnedItem,
                    screenShare = renderScreenShareFocus,
                    adaptiveMode = effectiveAdaptiveMode,
                    localParticipantSid = localSid,
                    previousState = layoutState,
                    nowMs = System.currentTimeMillis(),
                )

            // Video grid area with reaction overlay
            val isFullscreenFocus = layoutDecision.mode == LayoutMode.FOCUS
            Box(
                modifier =
                    Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .padding(if (isFullscreenFocus) 0.dp else 8.dp)
                        .testTag("layout-mode:${if (layoutDecision.mode == LayoutMode.FOCUS) "FOCUS" else "GRID"}"),
            ) {
                AdaptiveModeVideoArea(
                    effectiveAdaptiveMode = effectiveAdaptiveMode,
                    layoutDecision = layoutDecision,
                    participants = participants,
                    handRaisedMap = handRaisedMap,
                    controlsVisible = controlsVisible,
                    lang = lang,
                    onToggleControls = { controlsVisible = !controlsVisible },
                    onExitFocus = {
                        focusedItem = null
                        userPinnedItem = null
                    },
                    onFocusItem = { fi ->
                        focusedItem = fi
                        userPinnedItem = fi
                    },
                    onTogglePin = { fi ->
                        userPinnedItem = if (userPinnedItem == fi) null else fi
                    },
                )

                // Reaction overlay on top of video grid
                ReactionOverlay(reactions = reactions)

                // Persistent adaptive mode indicator (on top of everything)
                if (isAdaptiveModeEnabled && !isFullscreenFocus) {
                    AdaptiveModeIndicator(
                        adaptiveMode = adaptiveMode,
                        lang = lang,
                        modifier = Modifier.align(Alignment.TopEnd),
                    )
                }
            }

            // Control bar — hidden in fullscreen focus mode unless tapped
            if (focusedItem == null || controlsVisible) {
                Spacer(modifier = Modifier.height(8.dp))

                ControlBar(
                    micEnabled = micEnabled,
                    cameraEnabled = cameraEnabled,
                    isHandRaised = isHandRaised,
                    unreadCount = unreadCount,
                    participantCount = participants.size,
                    showReactionPicker = showReactionPicker,
                    adaptiveMode = effectiveAdaptiveMode,
                    isAdaptiveModeEnabled = isAdaptiveModeEnabled,
                    lang = lang,
                    onToggleMic = {
                        toggleMic(micEnabled, context, coroutineScope, micPermissionLauncher) {
                            micEnabled = it
                        }
                    },
                    onAudioPicker = {
                        inCallSettingsTab = 1
                        showInCallSettings = true
                    },
                    onToggleCamera = {
                        toggleCamera(cameraEnabled, context, coroutineScope, cameraPermissionLauncher) {
                            cameraEnabled = it
                        }
                    },
                    onToggleHandRaise = {
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                if (isHandRaised) {
                                    VisioManager.client.lowerHand()
                                } else {
                                    VisioManager.client.raiseHand()
                                }
                            } catch (e: Exception) {
                                Log.e(TAG, "Failed to toggle hand raise", e)
                            }
                        }
                    },
                    onReaction = { emoji ->
                        VisioManager.sendReaction(emoji)
                        showReactionPicker = false
                    },
                    onToggleReactionPicker = { showReactionPicker = !showReactionPicker },
                    onParticipants = { showParticipantList = true },
                    onSettings = {
                        inCallSettingsTab = 0
                        showInCallSettings = true
                    },
                    onChat = onChatOpen,
                    onHangUp = {
                        VisioManager.disconnect()
                        onHangUp()
                    },
                    onAdaptiveModeOverride = { mode ->
                        coroutineScope.launch(Dispatchers.IO) {
                            VisioManager.client.setAdaptiveModeOverride(mode)
                        }
                    },
                )

                Spacer(modifier = Modifier.height(8.dp))
            } // end control bar visibility
        }

        // Lobby: persistent banner when participants are waiting
        LobbyWaitingBanner(
            waitingParticipants = waitingParticipants,
            lang = lang,
            onAdmit = { participant ->
                VisioManager.admitParticipant(participant.id)
            },
            onView = {
                showParticipantList = true
            },
            modifier =
                Modifier
                    .align(Alignment.TopCenter)
                    .statusBarsPadding()
                    .padding(top = 8.dp, start = 16.dp, end = 16.dp),
        )
    }
}

@Composable
private fun LobbyWaitingBanner(
    waitingParticipants: List<WaitingParticipant>,
    lang: String,
    onAdmit: (WaitingParticipant) -> Unit,
    onView: () -> Unit,
    modifier: Modifier = Modifier,
) {
    AnimatedVisibility(
        visible = waitingParticipants.isNotEmpty(),
        enter = slideInVertically { -it } + fadeIn(),
        exit = slideOutVertically { -it } + fadeOut(),
        modifier = modifier,
    ) {
        if (waitingParticipants.isNotEmpty()) {
            val first = waitingParticipants.first()
            val message =
                if (waitingParticipants.size == 1) {
                    Strings.t("lobby.joinRequest", lang).replace("{{name}}", first.username)
                } else {
                    Strings.t("lobby.joinRequest", lang).replace("{{name}}", first.username) +
                        " (+${waitingParticipants.size - 1})"
                }
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .shadow(8.dp, RoundedCornerShape(12.dp))
                        .background(VisioColors.PrimaryDark75, RoundedCornerShape(12.dp))
                        .padding(horizontal = 12.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    text = message,
                    color = VisioColors.White,
                    fontSize = 14.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Button(
                    onClick = { onAdmit(first) },
                    colors =
                        ButtonDefaults.buttonColors(
                            containerColor = VisioColors.Primary500,
                        ),
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier.height(32.dp),
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 0.dp),
                ) {
                    Text(
                        text = Strings.t("lobby.admit", lang),
                        fontSize = 12.sp,
                    )
                }
                OutlinedButton(
                    onClick = onView,
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier.height(32.dp),
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 0.dp),
                ) {
                    Text(
                        text = Strings.t("lobby.view", lang),
                        fontSize = 12.sp,
                        color = VisioColors.White,
                    )
                }
            }
        }
    }
}

@Composable
private fun CallPiPView(
    participants: List<ParticipantInfo>,
    activeSpeakers: List<String>,
) {
    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .background(VisioColors.PrimaryDark50),
        contentAlignment = Alignment.Center,
    ) {
        val activeSpeakerSid = activeSpeakers.firstOrNull()
        val speaker = participants.find { it.sid == activeSpeakerSid } ?: participants.firstOrNull()
        if (speaker != null) {
            ParticipantTile(
                participant = speaker,
                isActiveSpeaker = false,
                handRaisePosition = 0,
                onClick = {},
            )
        }
    }
}

@Suppress("kotlin:S107")
@Composable
private fun AdaptiveModeVideoArea(
    effectiveAdaptiveMode: AdaptiveMode,
    layoutDecision: LayoutDecision,
    participants: List<ParticipantInfo>,
    handRaisedMap: Map<String, Int>,
    controlsVisible: Boolean,
    lang: String,
    onToggleControls: () -> Unit,
    onExitFocus: () -> Unit,
    onFocusItem: (FocusItem) -> Unit,
    onTogglePin: (FocusItem) -> Unit,
) {
    when (effectiveAdaptiveMode) {
        AdaptiveMode.CAR -> {
            val speaker = layoutDecision.mainTile?.participant ?: participants.firstOrNull()
            CarModeView(
                speakerName = speaker?.name ?: speaker?.identity ?: "",
                lang = lang,
            )
        }
        AdaptiveMode.PEDESTRIAN -> {
            val mainParticipant = layoutDecision.mainTile?.participant ?: participants.firstOrNull()
            PedestrianModeView(
                participant = mainParticipant,
                speakerIndicatorSid = layoutDecision.speakerIndicatorSid,
                handRaisedMap = handRaisedMap,
            )
        }
        AdaptiveMode.OFFICE -> {
            OfficeVideoArea(
                layoutDecision = layoutDecision,
                participants = participants,
                handRaisedMap = handRaisedMap,
                controlsVisible = controlsVisible,
                onToggleControls = onToggleControls,
                onExitFocus = onExitFocus,
                onFocusItem = onFocusItem,
                onTogglePin = onTogglePin,
            )
        }
    }
}

@Composable
private fun OfficeVideoArea(
    layoutDecision: LayoutDecision,
    participants: List<ParticipantInfo>,
    handRaisedMap: Map<String, Int>,
    controlsVisible: Boolean,
    onToggleControls: () -> Unit,
    onExitFocus: () -> Unit,
    onFocusItem: (FocusItem) -> Unit,
    onTogglePin: (FocusItem) -> Unit,
) {
    val displayItems = buildDisplayItems(participants)
    if (layoutDecision.mode == LayoutMode.FOCUS && layoutDecision.mainTile != null) {
        OfficeFocusLayout(
            focusedDisplayItem = layoutDecision.mainTile,
            secondaryTiles = layoutDecision.secondaryTiles,
            speakerIndicatorSid = layoutDecision.speakerIndicatorSid,
            pinnedIndicatorSid = layoutDecision.pinnedIndicatorSid,
            handRaisedMap = handRaisedMap,
            controlsVisible = controlsVisible,
            onToggleControls = onToggleControls,
            onExitFocus = onExitFocus,
            onFocusItem = onFocusItem,
            onTogglePin = onTogglePin,
        )
    } else {
        OfficeGridLayout(
            displayItems = displayItems,
            speakerIndicatorSid = layoutDecision.speakerIndicatorSid,
            handRaisedMap = handRaisedMap,
            onFocusItem = onFocusItem,
            onTogglePin = onTogglePin,
        )
    }
}

@Composable
private fun CarModeView(
    speakerName: String,
    lang: String,
) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_mic_line),
                contentDescription = null,
                tint = VisioColors.Primary500,
                modifier = Modifier.size(64.dp),
            )
            Spacer(modifier = Modifier.height(24.dp))
            Text(
                text = speakerName,
                color = Color.White,
                fontSize = 32.sp,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 32.dp),
            )
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = Strings.t("adaptive.audioOnly", lang),
                color = Color.White.copy(alpha = 0.6f),
                fontSize = 16.sp,
            )
        }
    }
}

@Composable
private fun PedestrianModeView(
    participant: ParticipantInfo?,
    speakerIndicatorSid: String?,
    handRaisedMap: Map<String, Int>,
) {
    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .clip(RoundedCornerShape(8.dp))
                .testTag("main-tile:${participant?.sid ?: ""}"),
    ) {
        if (participant != null) {
            ParticipantTile(
                participant = participant,
                isActiveSpeaker = speakerIndicatorSid == participant.sid,
                handRaisePosition = handRaisedMap[participant.sid] ?: 0,
                onClick = {},
            )
        }
    }
}

@Suppress("kotlin:S107", "kotlin:S3776")
@Composable
private fun OfficeFocusLayout(
    focusedDisplayItem: DisplayItem,
    secondaryTiles: List<DisplayItem>,
    speakerIndicatorSid: String?,
    pinnedIndicatorSid: String?,
    handRaisedMap: Map<String, Int>,
    controlsVisible: Boolean,
    onToggleControls: () -> Unit,
    onExitFocus: () -> Unit,
    onFocusItem: (FocusItem) -> Unit,
    onTogglePin: (FocusItem) -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize()) {
        // Main focused item
        Box(
            modifier =
                Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(8.dp))
                    .testTag("main-tile:${focusedDisplayItem.participant.sid}")
                    .background(VisioColors.PrimaryDark50)
                    .clickable { onToggleControls() },
        ) {
            val hasTrack =
                focusedDisplayItem.trackSid != null &&
                    if (focusedDisplayItem.isScreenShare) {
                        focusedDisplayItem.participant.hasScreenShare
                    } else {
                        focusedDisplayItem.participant.hasVideo
                    }
            if (hasTrack) {
                AndroidView(
                    factory = { ctx -> VideoSurfaceView(ctx, focusedDisplayItem.trackSid!!) },
                    update = { view -> view.requestLayout() },
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                ParticipantTile(
                    participant = focusedDisplayItem.participant,
                    isActiveSpeaker = speakerIndicatorSid == focusedDisplayItem.participant.sid,
                    handRaisePosition = handRaisedMap[focusedDisplayItem.participant.sid] ?: 0,
                    isPinned = pinnedIndicatorSid == focusedDisplayItem.participant.sid,
                    onClick = { onToggleControls() },
                )
            }
            // Overlay: name + screen share icon
            Row(
                modifier =
                    Modifier
                        .align(Alignment.BottomStart)
                        .padding(12.dp)
                        .background(Color.Black.copy(alpha = 0.5f), RoundedCornerShape(8.dp))
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                if (focusedDisplayItem.isScreenShare) {
                    Icon(
                        painter = painterResource(R.drawable.ic_screen_share),
                        contentDescription = null,
                        tint = Color(0xFF4FC3F7),
                        modifier = Modifier.size(16.dp),
                    )
                }
                Text(
                    text = focusedDisplayItem.label,
                    color = Color.White,
                    fontSize = 14.sp,
                )
            }

            // Pin indicator (top-left)
            if (pinnedIndicatorSid == focusedDisplayItem.participant.sid) {
                Box(
                    modifier =
                        Modifier
                            .align(Alignment.TopStart)
                            .padding(8.dp)
                            .size(28.dp)
                            .background(Color.Black.copy(alpha = 0.6f), CircleShape)
                            .padding(4.dp),
                ) {
                    Text(
                        text = "\uD83D\uDCCC",
                        fontSize = 12.sp,
                        modifier = Modifier.align(Alignment.Center),
                    )
                }
            }

            // Close/exit focus button (top-right)
            IconButton(
                onClick = onExitFocus,
                modifier =
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(8.dp)
                        .size(36.dp)
                        .background(
                            Color.Black.copy(alpha = 0.5f),
                            CircleShape,
                        ),
            ) {
                Icon(
                    imageVector = Icons.Outlined.Close,
                    contentDescription = "Exit focus",
                    tint = Color.White,
                    modifier = Modifier.size(22.dp),
                )
            }
        }

        // Thumbnail bar — hidden in fullscreen focus mode
        if (controlsVisible && secondaryTiles.isNotEmpty()) {
            Spacer(modifier = Modifier.height(8.dp))
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .height(100.dp),
            ) {
                secondaryTiles.forEachIndexed { index, item ->
                    Box(
                        modifier =
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .clip(RoundedCornerShape(8.dp))
                                .testTag("secondary-tile-$index:${item.participant.sid}"),
                    ) {
                        ParticipantTile(
                            participant = item.participant,
                            isActiveSpeaker = speakerIndicatorSid == item.participant.sid,
                            handRaisePosition = handRaisedMap[item.participant.sid] ?: 0,
                            isScreenShare = item.isScreenShare,
                            isPinned = pinnedIndicatorSid == item.participant.sid,
                            onClick = {
                                onFocusItem(FocusItem(item.participant.sid, item.source))
                            },
                            onLongPress = {
                                onTogglePin(FocusItem(item.participant.sid, item.source))
                            },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun OfficeGridLayout(
    displayItems: List<DisplayItem>,
    speakerIndicatorSid: String?,
    handRaisedMap: Map<String, Int>,
    onFocusItem: (FocusItem) -> Unit,
    onTogglePin: (FocusItem) -> Unit,
) {
    val count = displayItems.size
    if (count > 0) {
        BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
            val isLandscape = maxWidth > maxHeight
            val columnCount =
                when {
                    count == 1 -> 1
                    isLandscape -> minOf(count, 3)
                    count <= 2 -> 1
                    else -> 2
                }
            val rowCount = maxOf(1, (count + columnCount - 1) / columnCount)
            val tileHeight = (maxHeight - 8.dp * (rowCount - 1)) / rowCount

            Column(
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxSize(),
            ) {
                for (rowStart in 0 until count step columnCount) {
                    val rowEnd = minOf(rowStart + columnCount, count)
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .height(tileHeight),
                    ) {
                        for (idx in rowStart until rowEnd) {
                            val item = displayItems[idx]
                            Box(
                                modifier =
                                    Modifier
                                        .weight(1f)
                                        .fillMaxHeight()
                                        .clip(RoundedCornerShape(8.dp))
                                        .testTag("grid-tile-$idx:${item.participant.sid}"),
                            ) {
                                ParticipantTile(
                                    participant = item.participant,
                                    isActiveSpeaker = speakerIndicatorSid == item.participant.sid,
                                    handRaisePosition = handRaisedMap[item.participant.sid] ?: 0,
                                    isScreenShare = item.isScreenShare,
                                    onClick = {
                                        onFocusItem(FocusItem(item.participant.sid, item.source))
                                    },
                                    onLongPress = {
                                        onTogglePin(FocusItem(item.participant.sid, item.source))
                                    },
                                )
                                // Fullscreen icon overlay for screen share tiles
                                if (item.isScreenShare) {
                                    IconButton(
                                        onClick = {
                                            onFocusItem(FocusItem(item.participant.sid, item.source))
                                        },
                                        modifier =
                                            Modifier
                                                .align(Alignment.TopEnd)
                                                .padding(4.dp)
                                                .size(32.dp)
                                                .background(
                                                    Color.Black.copy(alpha = 0.5f),
                                                    CircleShape,
                                                ),
                                    ) {
                                        Icon(
                                            imageVector = Icons.Outlined.Fullscreen,
                                            contentDescription = "Fullscreen",
                                            tint = Color.White,
                                            modifier = Modifier.size(20.dp),
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun AdaptiveModeIndicator(
    adaptiveMode: AdaptiveMode,
    lang: String,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier =
            modifier
                .padding(8.dp)
                .background(Color.Black.copy(alpha = 0.6f), RoundedCornerShape(12.dp))
                .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        val (modeIcon, modeKey) =
            when (adaptiveMode) {
                uniffi.visio.AdaptiveMode.OFFICE -> "🏢" to "adaptive.office"
                uniffi.visio.AdaptiveMode.PEDESTRIAN -> "🚶" to "adaptive.pedestrian"
                uniffi.visio.AdaptiveMode.CAR -> "🚗" to "adaptive.car"
            }
        Text(text = modeIcon, fontSize = 12.sp)
        Text(
            text = Strings.t(modeKey, lang),
            color = Color.White,
            fontSize = 11.sp,
        )
    }
}

@Suppress("kotlin:S107")
@Composable
private fun ControlBar(
    micEnabled: Boolean,
    cameraEnabled: Boolean,
    isHandRaised: Boolean,
    unreadCount: Int,
    participantCount: Int,
    showReactionPicker: Boolean,
    adaptiveMode: AdaptiveMode,
    isAdaptiveModeEnabled: Boolean,
    lang: String,
    onToggleMic: () -> Unit,
    onAudioPicker: () -> Unit,
    onToggleCamera: () -> Unit,
    onToggleHandRaise: () -> Unit,
    onReaction: (String) -> Unit,
    onToggleReactionPicker: () -> Unit,
    onParticipants: () -> Unit,
    onSettings: () -> Unit,
    onChat: () -> Unit,
    onHangUp: () -> Unit,
    onAdaptiveModeOverride: (AdaptiveMode?) -> Unit,
) {
    var showOverflow by remember { mutableStateOf(false) }
    var adaptiveModeOverride by remember { mutableStateOf<AdaptiveMode?>(null) }

    Column(
        modifier = Modifier.fillMaxWidth(),
    ) {
        if (showReactionPicker) {
            ReactionPickerRow(onReaction = onReaction)
        }

        if (showOverflow) {
            OverflowMenu(
                isHandRaised = isHandRaised,
                lang = lang,
                isAdaptiveModeEnabled = isAdaptiveModeEnabled,
                adaptiveModeOverride = adaptiveModeOverride,
                onToggleHandRaise = {
                    onToggleHandRaise()
                    showOverflow = false
                },
                onReactionPicker = {
                    showOverflow = false
                    onToggleReactionPicker()
                },
                onSettings = {
                    showOverflow = false
                    onSettings()
                },
                onAdaptiveModeOverride = { mode ->
                    adaptiveModeOverride = mode
                    onAdaptiveModeOverride(mode)
                },
            )
        }

        ControlBarButtons(
            micEnabled = micEnabled,
            cameraEnabled = cameraEnabled,
            unreadCount = unreadCount,
            participantCount = participantCount,
            adaptiveMode = adaptiveMode,
            isAdaptiveModeEnabled = isAdaptiveModeEnabled,
            showOverflow = showOverflow,
            lang = lang,
            onToggleMic = onToggleMic,
            onAudioPicker = onAudioPicker,
            onToggleCamera = onToggleCamera,
            onParticipants = onParticipants,
            onChat = onChat,
            onToggleOverflow = { showOverflow = !showOverflow },
            onHangUp = onHangUp,
        )
    }
}

@Composable
private fun ReactionPickerRow(onReaction: (String) -> Unit) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 4.dp)
                .background(Color(0xCC000000), RoundedCornerShape(12.dp))
                .padding(horizontal = 8.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        REACTION_EMOJIS.forEach { (id, emoji) ->
            Text(
                text = emoji,
                fontSize = 28.sp,
                modifier =
                    Modifier
                        .clickable { onReaction(id) }
                        .padding(4.dp),
            )
        }
    }
}

@Composable
private fun OverflowMenu(
    isHandRaised: Boolean,
    lang: String,
    isAdaptiveModeEnabled: Boolean,
    adaptiveModeOverride: AdaptiveMode?,
    onToggleHandRaise: () -> Unit,
    onReactionPicker: () -> Unit,
    onSettings: () -> Unit,
    onAdaptiveModeOverride: (AdaptiveMode?) -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 4.dp)
                .background(Color(0xCC000000), RoundedCornerShape(12.dp))
                .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OverflowHandRaiseItem(isHandRaised, lang, onToggleHandRaise)
        OverflowReactionItem(onReactionPicker)
        OverflowSettingsItem(lang, onSettings)
    }

    if (isAdaptiveModeEnabled) {
        AdaptiveModeOverridePicker(
            lang = lang,
            adaptiveModeOverride = adaptiveModeOverride,
            onAdaptiveModeOverride = onAdaptiveModeOverride,
        )
    }
}

@Composable
private fun OverflowHandRaiseItem(
    isHandRaised: Boolean,
    lang: String,
    onToggleHandRaise: () -> Unit,
) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.clickable { onToggleHandRaise() }.padding(horizontal = 8.dp),
    ) {
        IconButton(
            onClick = onToggleHandRaise,
            modifier =
                Modifier
                    .size(38.dp)
                    .background(
                        if (isHandRaised) VisioColors.HandRaise else VisioColors.PrimaryDark100,
                        RoundedCornerShape(8.dp),
                    )
                    .testTag("call_hand_raise_button"),
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_hand),
                contentDescription =
                    if (isHandRaised) Strings.t("control.lowerHand", lang) else Strings.t("control.raiseHand", lang),
                tint = if (isHandRaised) Color.Black else VisioColors.White,
                modifier = Modifier.size(20.dp),
            )
        }
        Text(
            text = if (isHandRaised) Strings.t("control.lowerHand", lang) else Strings.t("control.raiseHand", lang),
            color = VisioColors.White,
            fontSize = 10.sp,
            maxLines = 1,
        )
    }
}

@Composable
private fun OverflowReactionItem(onReactionPicker: () -> Unit) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.clickable { onReactionPicker() }.padding(horizontal = 8.dp),
    ) {
        IconButton(
            onClick = onReactionPicker,
            modifier =
                Modifier
                    .size(38.dp)
                    .background(VisioColors.PrimaryDark100, RoundedCornerShape(8.dp)),
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_emotion_line),
                contentDescription = "Reaction",
                tint = VisioColors.White,
                modifier = Modifier.size(20.dp),
            )
        }
        Text(
            text = "Reaction",
            color = VisioColors.White,
            fontSize = 10.sp,
            maxLines = 1,
        )
    }
}

@Composable
private fun OverflowSettingsItem(
    lang: String,
    onSettings: () -> Unit,
) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.clickable { onSettings() }.padding(horizontal = 8.dp),
    ) {
        IconButton(
            onClick = onSettings,
            modifier =
                Modifier
                    .size(38.dp)
                    .background(VisioColors.PrimaryDark100, RoundedCornerShape(8.dp))
                    .testTag("call_settings_button"),
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_settings_3_line),
                contentDescription = Strings.t("settings.incall", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(20.dp),
            )
        }
        Text(
            text = Strings.t("settings.incall", lang),
            color = VisioColors.White,
            fontSize = 10.sp,
            maxLines = 1,
        )
    }
}

@Composable
private fun AdaptiveModeOverridePicker(
    lang: String,
    adaptiveModeOverride: AdaptiveMode?,
    onAdaptiveModeOverride: (AdaptiveMode?) -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 4.dp)
                .background(Color(0xCC000000), RoundedCornerShape(12.dp))
                .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Text(
            text = Strings.t("adaptive.override", lang),
            color = VisioColors.White,
            fontSize = 11.sp,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.padding(bottom = 6.dp),
        )
        Row(
            horizontalArrangement = Arrangement.SpaceEvenly,
            modifier = Modifier.fillMaxWidth(),
        ) {
            val modeOptions =
                listOf<Pair<AdaptiveMode?, String>>(
                    null to Strings.t("adaptive.auto", lang),
                    AdaptiveMode.OFFICE to Strings.t("adaptive.office", lang),
                    AdaptiveMode.PEDESTRIAN to Strings.t("adaptive.pedestrian", lang),
                    AdaptiveMode.CAR to Strings.t("adaptive.car", lang),
                )
            modeOptions.forEach { (mode, label) ->
                val isSelected = mode == adaptiveModeOverride
                Text(
                    text = label,
                    color = if (isSelected) Color.Black else VisioColors.White,
                    fontSize = 11.sp,
                    fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Normal,
                    modifier =
                        Modifier
                            .background(
                                if (isSelected) VisioColors.Primary500 else VisioColors.PrimaryDark100,
                                RoundedCornerShape(16.dp),
                            )
                            .clickable { onAdaptiveModeOverride(mode) }
                            .padding(horizontal = 10.dp, vertical = 6.dp),
                    maxLines = 1,
                )
            }
        }
    }
}

@Suppress("kotlin:S107")
@Composable
private fun ControlBarButtons(
    micEnabled: Boolean,
    cameraEnabled: Boolean,
    unreadCount: Int,
    participantCount: Int,
    adaptiveMode: AdaptiveMode,
    isAdaptiveModeEnabled: Boolean,
    showOverflow: Boolean,
    lang: String,
    onToggleMic: () -> Unit,
    onAudioPicker: () -> Unit,
    onToggleCamera: () -> Unit,
    onParticipants: () -> Unit,
    onChat: () -> Unit,
    onToggleOverflow: () -> Unit,
    onHangUp: () -> Unit,
) {
    val effectiveMode = if (isAdaptiveModeEnabled) adaptiveMode else AdaptiveMode.OFFICE
    val isLargeButtons = effectiveMode != AdaptiveMode.OFFICE
    val btnSize = if (isLargeButtons) 96.dp else 38.dp
    val iconSize = if (isLargeButtons) 48.dp else 20.dp
    val cornerRadius = if (isLargeButtons) 16.dp else 8.dp

    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp)
                .background(VisioColors.PrimaryDark75, RoundedCornerShape(16.dp))
                .padding(horizontal = 6.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        MicButtonGroup(micEnabled, btnSize, iconSize, cornerRadius, isLargeButtons, lang, onToggleMic, onAudioPicker)

        if (adaptiveMode != AdaptiveMode.CAR) {
            CameraButton(cameraEnabled, btnSize, iconSize, cornerRadius, lang, onToggleCamera)
        }

        if (adaptiveMode == AdaptiveMode.OFFICE) {
            ParticipantsButton(participantCount, btnSize, iconSize, cornerRadius, lang, onParticipants)
            ChatButton(unreadCount, btnSize, iconSize, cornerRadius, lang, onChat)
            OverflowButton(showOverflow, btnSize, iconSize, cornerRadius, onToggleOverflow)
        }

        HangUpButton(btnSize, iconSize, cornerRadius, lang, onHangUp)
    }
}

@Suppress("kotlin:S107")
@Composable
private fun MicButtonGroup(
    micEnabled: Boolean,
    btnSize: androidx.compose.ui.unit.Dp,
    iconSize: androidx.compose.ui.unit.Dp,
    cornerRadius: androidx.compose.ui.unit.Dp,
    isLargeButtons: Boolean,
    lang: String,
    onToggleMic: () -> Unit,
    onAudioPicker: () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .background(
                    if (micEnabled) VisioColors.PrimaryDark100 else VisioColors.Error200,
                    RoundedCornerShape(cornerRadius),
                ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(
            onClick = onToggleMic,
            modifier = Modifier.size(btnSize).testTag("call_mic_button"),
        ) {
            Icon(
                painter = painterResource(if (micEnabled) R.drawable.ri_mic_line else R.drawable.ri_mic_off_line),
                contentDescription = if (micEnabled) Strings.t("control.mute", lang) else Strings.t("control.unmute", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(iconSize),
            )
        }
        val chevronHeight = if (isLargeButtons) 96.dp else 38.dp
        val chevronWidth = if (isLargeButtons) 40.dp else 22.dp
        val chevronIconSize = if (isLargeButtons) 24.dp else 14.dp
        IconButton(
            onClick = onAudioPicker,
            modifier = Modifier.size(chevronWidth, chevronHeight),
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_arrow_up_s_line),
                contentDescription = Strings.t("control.audioDevices", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(chevronIconSize),
            )
        }
    }
}

@Composable
private fun CameraButton(
    cameraEnabled: Boolean,
    btnSize: androidx.compose.ui.unit.Dp,
    iconSize: androidx.compose.ui.unit.Dp,
    cornerRadius: androidx.compose.ui.unit.Dp,
    lang: String,
    onToggleCamera: () -> Unit,
) {
    IconButton(
        onClick = onToggleCamera,
        modifier =
            Modifier
                .size(btnSize)
                .background(
                    if (cameraEnabled) VisioColors.PrimaryDark100 else VisioColors.Error200,
                    RoundedCornerShape(cornerRadius),
                )
                .testTag("call_camera_button"),
    ) {
        Icon(
            painter = painterResource(if (cameraEnabled) R.drawable.ri_video_on_line else R.drawable.ri_video_off_line),
            contentDescription = if (cameraEnabled) Strings.t("control.camOff", lang) else Strings.t("control.camOn", lang),
            tint = VisioColors.White,
            modifier = Modifier.size(iconSize),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ParticipantsButton(
    participantCount: Int,
    btnSize: androidx.compose.ui.unit.Dp,
    iconSize: androidx.compose.ui.unit.Dp,
    cornerRadius: androidx.compose.ui.unit.Dp,
    lang: String,
    onParticipants: () -> Unit,
) {
    IconButton(
        onClick = onParticipants,
        modifier =
            Modifier
                .size(btnSize)
                .background(VisioColors.PrimaryDark100, RoundedCornerShape(cornerRadius))
                .testTag("call_participants_button"),
    ) {
        BadgedBox(
            badge = {
                if (participantCount > 0) {
                    Badge(
                        containerColor = VisioColors.Primary500,
                        contentColor = VisioColors.White,
                    ) {
                        Text(text = "$participantCount", fontSize = 10.sp)
                    }
                }
            },
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_group_line),
                contentDescription = Strings.t("participants.title", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(iconSize),
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ChatButton(
    unreadCount: Int,
    btnSize: androidx.compose.ui.unit.Dp,
    iconSize: androidx.compose.ui.unit.Dp,
    cornerRadius: androidx.compose.ui.unit.Dp,
    lang: String,
    onChat: () -> Unit,
) {
    IconButton(
        onClick = onChat,
        modifier =
            Modifier
                .size(btnSize)
                .background(VisioColors.PrimaryDark100, RoundedCornerShape(cornerRadius))
                .testTag("call_chat_button"),
    ) {
        BadgedBox(
            badge = {
                if (unreadCount > 0) {
                    Badge(
                        containerColor = VisioColors.Error500,
                        contentColor = VisioColors.White,
                    ) {
                        Text(text = "$unreadCount", fontSize = 10.sp)
                    }
                }
            },
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_chat_1_line),
                contentDescription = Strings.t("chat", lang),
                tint = VisioColors.White,
                modifier = Modifier.size(iconSize),
            )
        }
    }
}

@Composable
private fun OverflowButton(
    showOverflow: Boolean,
    btnSize: androidx.compose.ui.unit.Dp,
    iconSize: androidx.compose.ui.unit.Dp,
    cornerRadius: androidx.compose.ui.unit.Dp,
    onToggleOverflow: () -> Unit,
) {
    IconButton(
        onClick = onToggleOverflow,
        modifier =
            Modifier
                .size(btnSize)
                .background(
                    if (showOverflow) VisioColors.Primary500 else VisioColors.PrimaryDark100,
                    RoundedCornerShape(cornerRadius),
                ),
    ) {
        Icon(
            painter = painterResource(R.drawable.ri_more_2_fill),
            contentDescription = "More",
            tint = VisioColors.White,
            modifier = Modifier.size(iconSize),
        )
    }
}

@Composable
private fun HangUpButton(
    btnSize: androidx.compose.ui.unit.Dp,
    iconSize: androidx.compose.ui.unit.Dp,
    cornerRadius: androidx.compose.ui.unit.Dp,
    lang: String,
    onHangUp: () -> Unit,
) {
    IconButton(
        onClick = onHangUp,
        modifier =
            Modifier
                .size(btnSize)
                .background(VisioColors.Error500, RoundedCornerShape(cornerRadius))
                .testTag("call_hangup_button"),
    ) {
        Icon(
            painter = painterResource(R.drawable.ri_phone_fill),
            contentDescription = Strings.t("control.leave", lang),
            tint = VisioColors.White,
            modifier = Modifier.size(iconSize),
        )
    }
}

@Suppress("kotlin:S3776")
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ParticipantTile(
    participant: ParticipantInfo,
    isActiveSpeaker: Boolean,
    handRaisePosition: Int,
    isScreenShare: Boolean = false,
    isPinned: Boolean = false,
    onClick: () -> Unit,
    onLongPress: (() -> Unit)? = null,
) {
    val lang = VisioManager.currentLang
    val bwMode by VisioManager.bandwidthMode.collectAsState()
    val name = participant.name ?: participant.identity
    // Deterministic hue from name (reserved for avatar background, currently unused)

    @Suppress("kotlin:S1481")
    val hue = name.fold(0) { acc, c -> acc + c.code }.absoluteValue % 360

    @Suppress("kotlin:S1481")
    val avatarColor = Color.hsl(hue.toFloat(), 0.5f, 0.35f)

    val borderColor = if (isActiveSpeaker && !isScreenShare) VisioColors.Primary500 else Color.Transparent
    val speakerTestTag =
        if (isActiveSpeaker && !isScreenShare) {
            Modifier.testTag("speaker-border:${participant.sid}")
        } else {
            Modifier
        }
    val borderMod =
        if (isActiveSpeaker && !isScreenShare) {
            Modifier
                .border(2.dp, borderColor, RoundedCornerShape(8.dp))
                .shadow(8.dp, RoundedCornerShape(8.dp), ambientColor = VisioColors.Primary500)
                .then(speakerTestTag)
        } else {
            Modifier
        }

    // For screen share tiles, use screenShareTrackSid; for camera tiles, use videoTrackSid
    val hasTrack =
        if (isScreenShare) {
            participant.hasScreenShare && participant.screenShareTrackSid != null
        } else {
            participant.hasVideo && participant.videoTrackSid != null
        }
    val trackSid = if (isScreenShare) participant.screenShareTrackSid else participant.videoTrackSid

    // When bandwidth is degraded, video tracks are paused by the SDK.
    // Show a placeholder instead of a black frame.
    val videoPausedByBandwidth = hasTrack && !isScreenShare && when (bwMode) {
        BandwidthMode.AUDIO_ONLY -> true
        BandwidthMode.REDUCED_VIDEO -> !isActiveSpeaker
        else -> false
    }

    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .then(borderMod)
                .clip(RoundedCornerShape(8.dp))
                .background(VisioColors.PrimaryDark50)
                .combinedClickable(
                    onClick = onClick,
                    onLongClick = onLongPress,
                ),
    ) {
        // Video surface or avatar fallback
        if (hasTrack && trackSid != null && !videoPausedByBandwidth) {
            androidx.compose.runtime.key(trackSid) {
                AndroidView(
                    factory = { ctx -> VideoSurfaceView(ctx, trackSid) },
                    update = { view ->
                        // Force layout pass when SurfaceView is first inserted
                        // into the Compose hierarchy. Without this, the surface
                        // may not be visible until the user touches the screen.
                        view.requestLayout()
                    },
                    modifier = Modifier.fillMaxSize(),
                )
            }
        } else if (videoPausedByBandwidth) {
            // Video paused due to bandwidth degradation
            Box(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1A1A2E)),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Icon(
                        imageVector = Icons.Outlined.VideocamOff,
                        contentDescription = null,
                        tint = Color(0xFFFF9800),
                        modifier = Modifier.size(32.dp),
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = name,
                        color = Color(0xFF888888),
                        fontSize = 13.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = Strings.t("bandwidth.videoPaused", lang),
                        color = Color(0xFFFF9800),
                        fontSize = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        } else {
            Box(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .background(Color(0xFF1A1A2E)),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Icon(
                        imageVector = Icons.Outlined.VideocamOff,
                        contentDescription = null,
                        tint = Color(0xFF666666),
                        modifier = Modifier.size(32.dp),
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = name,
                        color = Color(0xFF888888),
                        fontSize = 13.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }

        // Pin indicator (top-right)
        if (isPinned) {
            Box(
                modifier =
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(6.dp)
                        .size(24.dp)
                        .background(Color.Black.copy(alpha = 0.6f), CircleShape)
                        .padding(4.dp)
                        .testTag("pin-indicator:${participant.sid}"),
            ) {
                Text(
                    text = "\uD83D\uDCCC",
                    fontSize = 12.sp,
                    modifier = Modifier.align(Alignment.Center),
                )
            }
        }

        // Metadata bar at bottom
        Row(
            modifier =
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth()
                    .background(Color(0x99000000))
                    .padding(horizontal = 8.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            // Screen share indicator
            if (isScreenShare) {
                Icon(
                    painter = painterResource(R.drawable.ic_screen_share),
                    contentDescription = null,
                    tint = Color(0xFF4FC3F7),
                    modifier = Modifier.size(14.dp),
                )
            }

            // Mic muted indicator (not shown on screen share tiles)
            if (participant.isMuted && !isScreenShare) {
                Icon(
                    painter = painterResource(R.drawable.ri_mic_off_fill),
                    contentDescription = Strings.t("accessibility.muted", lang),
                    tint = VisioColors.Error500,
                    modifier = Modifier.size(14.dp),
                )
            }

            // Hand raise badge (not shown on screen share tiles)
            if (handRaisePosition > 0 && !isScreenShare) {
                Row(
                    modifier =
                        Modifier
                            .background(VisioColors.HandRaise, RoundedCornerShape(10.dp))
                            .padding(horizontal = 6.dp, vertical = 1.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ri_hand),
                        contentDescription = null,
                        tint = Color.Black,
                        modifier = Modifier.size(12.dp),
                    )
                    Text(
                        text = "$handRaisePosition",
                        color = Color.Black,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold,
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
                modifier = Modifier.weight(1f),
            )

            // Connection quality bars
            ConnectionQualityBars(participant.connectionQuality.name)
        }
    }
}

@Composable
private fun ConnectionQualityBars(quality: String) {
    val bars =
        when (quality) {
            "Excellent" -> 3
            "Good" -> 2
            "Poor" -> 1
            else -> 0
        }
    Row(
        horizontalArrangement = Arrangement.spacedBy(1.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        for (i in 1..3) {
            Box(
                modifier =
                    Modifier
                        .width(3.dp)
                        .height((i * 4 + 2).dp)
                        .background(
                            if (i <= bars) Color.Green else VisioColors.Greyscale400,
                            RoundedCornerShape(1.dp),
                        ),
            )
        }
    }
}

@Composable
private fun BandwidthBanner(mode: BandwidthMode) {
    if (mode == BandwidthMode.FULL) return
    val lang = VisioManager.currentLang
    val text = when (mode) {
        BandwidthMode.REDUCED_VIDEO -> Strings.t("bandwidth.reducedVideo", lang)
        BandwidthMode.AUDIO_ONLY -> Strings.t("bandwidth.audioOnly", lang)
        else -> return
    }
    Box(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(Color(0xFFFFF3E0))
                .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Text(
            text = text,
            color = Color(0xFFE65100),
            style = MaterialTheme.typography.bodySmall,
            fontWeight = FontWeight.Medium,
        )
    }
}

@Composable
private fun ConnectionStateBanner(
    state: ConnectionState,
    errorMessage: String?,
) {
    val lang = VisioManager.currentLang
    when {
        errorMessage != null -> {
            Box(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .background(VisioColors.Error200)
                        .padding(12.dp),
            ) {
                Text(
                    text = "${Strings.t("call.error", lang)}: $errorMessage",
                    color = VisioColors.Error500,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        state is ConnectionState.Connecting -> {
            Row(
                modifier = Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    color = VisioColors.Primary500,
                )
                Text(
                    "${Strings.t("status.connecting", lang)}...",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }
        }
        state is ConnectionState.Reconnecting -> {
            Row(
                modifier = Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    color = VisioColors.Primary500,
                )
                Text(
                    "${Strings.t("status.reconnecting", lang)} (${state.attempt})...",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }
        }
        // Connected / Disconnected: no banner
    }
}

@Composable
private fun WaitingScreen(onCancel: () -> Unit) {
    val lang = VisioManager.currentLang

    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .background(VisioColors.PrimaryDark50),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            CircularProgressIndicator(
                modifier = Modifier.size(48.dp),
                color = VisioColors.Primary500,
            )
            Spacer(modifier = Modifier.height(24.dp))
            Text(
                text = Strings.t("lobby.waiting", lang),
                style = MaterialTheme.typography.titleMedium,
                color = VisioColors.White,
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = Strings.t("lobby.waitingDesc", lang),
                style = MaterialTheme.typography.bodyMedium,
                color = VisioColors.Greyscale400,
            )
            Spacer(modifier = Modifier.height(24.dp))
            OutlinedButton(onClick = onCancel) {
                Text(Strings.t("lobby.cancel", lang))
            }
        }
    }
}

@Composable
private fun ReactionOverlay(reactions: List<ReactionData>) {
    val now = System.currentTimeMillis()
    val activeReactions = reactions.filter { now - it.timestamp < 3000L }

    // Periodically trigger recomposition to remove expired reactions
    @Suppress("kotlin:S1481")
    var tick by remember { mutableStateOf(0L) }
    LaunchedEffect(reactions.size) {
        if (reactions.isNotEmpty()) {
            delay(3100L)
            tick = System.currentTimeMillis()
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        activeReactions.forEach { reaction ->
            FloatingReaction(reaction = reaction, modifier = Modifier.align(Alignment.BottomStart))
        }
    }
}

@Composable
private fun FloatingReaction(
    reaction: ReactionData,
    modifier: Modifier = Modifier,
) {
    val emojiDisplay = REACTION_EMOJIS.find { it.first == reaction.emoji }?.second ?: reaction.emoji
    val density = LocalDensity.current
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    // Deterministic horizontal position based on reaction id (left 20% of screen)
    val xOffsetDp =
        remember(reaction.id) {
            ((reaction.id * 37 + 13) % (screenWidthDp * 20 / 100)).toInt()
        }

    val progress = remember { Animatable(0f) }

    LaunchedEffect(reaction.id) {
        progress.animateTo(
            targetValue = 1f,
            animationSpec = tween(durationMillis = 3000, easing = LinearEasing),
        )
    }

    val yOffset = with(density) { (-300.dp * progress.value).roundToPx() }
    val alphaValue =
        if (progress.value > 0.7f) {
            1f - ((progress.value - 0.7f) / 0.3f)
        } else {
            1f
        }

    Column(
        modifier =
            modifier
                .offset { IntOffset(with(density) { xOffsetDp.dp.roundToPx() }, yOffset) }
                .alpha(alphaValue),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = emojiDisplay,
            fontSize = 32.sp,
        )
        Text(
            text = reaction.participantName,
            color = VisioColors.White,
            fontSize = 10.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
            modifier =
                Modifier
                    .background(Color(0x99000000), RoundedCornerShape(4.dp))
                    .padding(horizontal = 4.dp, vertical = 1.dp),
        )
    }
}
