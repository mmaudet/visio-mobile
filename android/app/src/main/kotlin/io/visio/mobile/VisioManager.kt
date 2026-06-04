package io.visio.mobile

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import android.os.PowerManager
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.app.NotificationCompat
import io.visio.mobile.auth.OidcAuthManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.visio.AdaptiveMode
import uniffi.visio.ChatMessage
import uniffi.visio.ConnectionState
import uniffi.visio.Meeting
import uniffi.visio.ParticipantInfo
import uniffi.visio.RoomAccess
import uniffi.visio.SessionState
import uniffi.visio.TrackKind
import uniffi.visio.TrackSource
import uniffi.visio.VisioClient
import uniffi.visio.VisioEvent
import uniffi.visio.VisioEventListener
import uniffi.visio.WaitingParticipant

sealed class CalendarSyncResult {
    data class Success(val count: Int) : CalendarSyncResult()

    data class Error(val message: String) : CalendarSyncResult()
}

/**
 * Pick `http` for loopback/private hosts (Android emulator points at the host
 * dev backend via 10.0.2.2), `https` for real public hostnames.
 */
internal fun schemeFor(host: String): String {
    val h = host.substringBefore(':').lowercase()
    val isLocal =
        h == "localhost" || h == "127.0.0.1" || h == "10.0.2.2" || h == "10.0.3.2" ||
            h.startsWith("192.168.") || h.startsWith("10.") ||
            (h.startsWith("172.") && (16..31).any { h.startsWith("172.$it.") })
    return if (isLocal) "http" else "https"
}

internal fun meetBaseUrl(host: String): String = "${schemeFor(host)}://$host"

object VisioManager : VisioEventListener {
    const val MEETING_CHANNEL_ID = "meetings"

    // Library loaded and WebRTC initialized by VisioApplication.onCreate()
    private lateinit var _client: VisioClient
    val client: VisioClient get() = _client

    // IO scope for callbacks that call back into Rust (avoids nested block_on)
    private var scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    // Camera capture (Camera2 -> JNI -> NativeVideoSource)
    private var cameraCapture: CameraCapture? = null

    // Audio capture (AudioRecord -> JNI -> NativeAudioSource)
    private var audioCapture: AudioCapture? = null

    // Audio playout (Rust playout buffer -> JNI -> AudioTrack)
    private var audioPlayout: AudioPlayout? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private lateinit var appContext: Context

    private val _connectionState = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)
    val connectionState: StateFlow<ConnectionState> = _connectionState.asStateFlow()

    private val _participants = MutableStateFlow<List<ParticipantInfo>>(emptyList())
    val participants: StateFlow<List<ParticipantInfo>> = _participants.asStateFlow()

    private val _chatMessages = MutableStateFlow<List<ChatMessage>>(emptyList())
    val chatMessages: StateFlow<List<ChatMessage>> = _chatMessages.asStateFlow()

    private val _activeSpeakers = MutableStateFlow<List<String>>(emptyList())
    val activeSpeakers: StateFlow<List<String>> = _activeSpeakers.asStateFlow()

    // Hand raise: map of participant_sid -> queue position (0 = not raised)
    private val _handRaisedMap = MutableStateFlow<Map<String, Int>>(emptyMap())
    val handRaisedMap: StateFlow<Map<String, Int>> = _handRaisedMap.asStateFlow()

    // Unread chat message count
    private val _unreadCount = MutableStateFlow(0)
    val unreadCount: StateFlow<Int> = _unreadCount.asStateFlow()

    // Whether local hand is raised
    private val _isHandRaised = MutableStateFlow(false)
    val isHandRaised: StateFlow<Boolean> = _isHandRaised.asStateFlow()

    // Lobby: participants waiting for host approval
    private val _waitingParticipants = MutableStateFlow<List<WaitingParticipant>>(emptyList())
    val waitingParticipants: StateFlow<List<WaitingParticipant>> = _waitingParticipants.asStateFlow()

    // Lobby: notification banner for newly joined waiting participant
    private val _lobbyNotification = MutableStateFlow<WaitingParticipant?>(null)
    val lobbyNotification: StateFlow<WaitingParticipant?> = _lobbyNotification.asStateFlow()

    // Lobby: whether entry was denied by host
    private val _lobbyDenied = MutableStateFlow(false)
    val lobbyDenied: MutableStateFlow<Boolean> = _lobbyDenied

    // Room access management for restricted rooms
    var currentRoomId: String? = null
        private set

    private val _roomAccesses = MutableStateFlow<List<RoomAccess>>(emptyList())
    val roomAccesses: StateFlow<List<RoomAccess>> = _roomAccesses

    private var _currentAccessLevel: String = ""
    val currentAccessLevel: String get() = _currentAccessLevel

    // Screen share auto-focus: emits participant SID when a screen share track is subscribed
    private val _screenShareSubscribed = MutableStateFlow<String?>(null)
    val screenShareSubscribed: StateFlow<String?> = _screenShareSubscribed.asStateFlow()

    fun clearScreenShareSubscribed() {
        _screenShareSubscribed.value = null
    }

    // Emoji reactions
    private var reactionIdCounter = 0L
    private val _reactions = MutableStateFlow<List<ReactionData>>(emptyList())
    val reactions: StateFlow<List<ReactionData>> = _reactions.asStateFlow()

    // Calendar: upcoming meetings
    private val _upcomingMeetings = MutableStateFlow<List<Meeting>>(emptyList())
    val upcomingMeetings: StateFlow<List<Meeting>> = _upcomingMeetings.asStateFlow()

    // Calendar: loading state (true while first fetch in progress)
    private val _calendarLoading = MutableStateFlow(false)
    val calendarLoading: StateFlow<Boolean> = _calendarLoading.asStateFlow()

    // Calendar: sync result for UI feedback (snackbar)
    private val _calendarSyncResult = MutableStateFlow<CalendarSyncResult?>(null)
    val calendarSyncResult: StateFlow<CalendarSyncResult?> = _calendarSyncResult.asStateFlow()

    /** Clear the calendar sync result after the UI has consumed it. */
    fun clearCalendarSyncResult() {
        _calendarSyncResult.value = null
    }

    // Bandwidth degradation mode
    private val _bandwidthMode = MutableStateFlow(uniffi.visio.BandwidthMode.FULL)
    val bandwidthMode: StateFlow<uniffi.visio.BandwidthMode> = _bandwidthMode.asStateFlow()

    // Adaptive mode
    private val _adaptiveMode = MutableStateFlow(AdaptiveMode.OFFICE)
    val adaptiveMode: StateFlow<AdaptiveMode> = _adaptiveMode.asStateFlow()

    // Context detector for adaptive modes
    private var contextDetector: ContextDetector? = null

    // Track whether camera was on before CAR mode forced it off
    private var cameraWasEnabledBeforeCar = false

    // Grace period: don't let CAR mode disable camera right after connection
    // (gives camera-on-join time to activate)
    private var connectionTimestampMs = 0L
    private val CONNECTION_GRACE_MS = 5000L // 5 seconds after connect

    // Track previous audio device to restore after car mode
    private var previousAudioDevice: AudioDeviceInfo? = null

    // Bluetooth device removal callback for mid-session fallback
    private var bluetoothDeviceCallback: AudioDeviceCallback? = null

    // Audio focus monitoring for phone call detection
    private var audioFocusListener: AudioManager.OnAudioFocusChangeListener? = null
    private var wasPlayingBeforeFocusLoss = false

    // Deep link: pre-fill room URL on HomeScreen
    var pendingDeepLink: String? by mutableStateOf(null)

    // Room display name from deep link (e.g. ?visio=...)
    var pendingDeepLinkDisplayName: String? by mutableStateOf(null)

    // Error from deep link alias resolution
    var pendingDeepLinkError: String? by mutableStateOf(null)

    // Test deep link: connect directly with LiveKit URL + token (debug builds only)
    var pendingTestConnect: Triple<String, String, String?>? = null // (livekitUrl, token, mediaFile?)

    // Audio device preferences from lobby (applied by CallScreen after connection)
    var pendingOutputDevice: AudioDeviceInfo? = null
    var pendingInputDevice: AudioDeviceInfo? = null

    // Flag to prevent CallScreen from re-initializing after permission dialogs
    var callScreenInitialized = false
    var isTestConnection: Boolean = false

    // Media file capture for E2E testing (replaces synthetic audio/camera)
    private var mediaFileCapture: MediaFileCapture? = null

    // Observable state for language, theme, display name
    var currentLang by mutableStateOf("fr")
        private set
    var currentTheme by mutableStateOf("light")
        private set
    var displayName by mutableStateOf("")
        private set

    // Session state properties
    var isAuthenticated by mutableStateOf(false)
        private set
    var authenticatedDisplayName by mutableStateOf("")
        private set
    var authenticatedEmail by mutableStateOf("")
        private set
    var authenticatedMeetInstance by mutableStateOf("")
        private set

    lateinit var authManager: OidcAuthManager
        private set

    private var initialized = false

    fun initialize(context: Context) {
        if (initialized) return
        appContext = context.applicationContext
        val dataDir = context.filesDir.absolutePath
        _client = VisioClient(dataDir)
        _client.addListener(this)
        // Load persisted settings (lightweight, safe on main thread)
        try {
            val settings = _client.getSettings()
            currentLang = settings.language ?: "fr"
            currentTheme = settings.theme ?: "light"
            displayName = settings.displayName ?: ""
        } catch (e: Exception) {
            Log.e("VisioManager", "Failed to load persisted settings", e)
        }
        createNotificationChannels()
        initialized = true
        // Heavy initialization off the main thread to avoid ANR on Android 15/16
        scope.launch {
            // Load ONNX segmentation model for background blur
            try {
                val modelFile = java.io.File(context.cacheDir, "selfie_segmentation.onnx")
                if (!modelFile.exists()) {
                    context.assets.open("models/selfie_segmentation.onnx").use { input ->
                        modelFile.outputStream().use { output -> input.copyTo(output) }
                    }
                }
                _client.loadBlurModel(modelFile.absolutePath)
                Log.i("VisioManager", "Blur model loaded from ${modelFile.absolutePath}")
            } catch (e: Exception) {
                Log.e("VisioManager", "Failed to load blur model", e)
            }
            // Load cached meetings so badge shows without waiting for network
            try {
                val cached = _client.getUpcomingMeetings()
                if (cached.isNotEmpty()) {
                    _upcomingMeetings.value = cached
                }
            } catch (e: Exception) {
                Log.e("VisioManager", "Failed to load cached meetings", e)
            }
        }
    }

    private fun createNotificationChannels() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel =
                NotificationChannel(
                    MEETING_CHANNEL_ID,
                    "Réunions",
                    NotificationManager.IMPORTANCE_HIGH,
                ).apply {
                    description = "Notifications pour les réunions à venir"
                }
            val nm = appContext.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(channel)
        }
    }

    fun setTheme(theme: String) {
        currentTheme = theme
        scope.launch { client.setTheme(theme) }
    }

    fun setLanguage(lang: String) {
        currentLang = lang
        scope.launch { client.setLanguage(lang) }
    }

    fun updateDisplayName(name: String) {
        displayName = name
    }

    fun initAuth(context: Context) {
        authManager = OidcAuthManager(context)
        // Try to restore session on launch — use the meet instance the tokens
        // were minted for, not `firstOrNull()` (which could be any attacker
        // entry that landed in the user's instance list at some point).
        val savedAccess = authManager.getSavedAccessToken()
        val savedRefresh = authManager.getSavedRefreshToken()
        val savedInstance = authManager.getSavedMeetInstance()
        if (savedAccess != null && savedRefresh != null && savedInstance != null) {
            CoroutineScope(Dispatchers.IO).launch {
                try {
                    client.setTokens(meetBaseUrl(savedInstance), savedAccess, savedRefresh)
                    val state = client.getSessionState()
                    withContext(Dispatchers.Main) {
                        updateSessionFromState(state)
                    }
                } catch (e: Exception) {
                    // Access token may have expired during downtime — try refresh once.
                    try {
                        client.setTokens(meetBaseUrl(savedInstance), savedAccess, savedRefresh)
                        client.refreshTokens(savedInstance)
                        val state = client.getSessionState()
                        withContext(Dispatchers.Main) {
                            updateSessionFromState(state)
                        }
                    } catch (_: Exception) {
                        authManager.clearTokens()
                    }
                }
            }
        }
    }

    private fun updateSessionFromState(state: SessionState) {
        when (state) {
            is SessionState.Authenticated -> {
                isAuthenticated = true
                authenticatedDisplayName = state.displayName
                authenticatedEmail = state.email
                authenticatedMeetInstance = state.meetInstance
                if (displayName.isEmpty()) {
                    displayName = state.displayName
                }
            }
            is SessionState.Anonymous -> {
                isAuthenticated = false
                authenticatedDisplayName = ""
                authenticatedEmail = ""
                authenticatedMeetInstance = ""
            }
        }
    }

    fun exchangePkceCode(
        code: String,
        codeVerifier: String,
        meetInstance: String,
    ) {
        scope.launch {
            try {
                val pair = client.exchangePkceCode(meetInstance, code, codeVerifier)
                onTokensReceived(pair.access, pair.refresh, meetInstance)
            } catch (e: Exception) {
                Log.e("VISIO", "PKCE code exchange failed: ${e.message}")
            }
        }
    }

    fun onTokensReceived(
        access: String,
        refresh: String,
        meetInstance: String,
    ) {
        authManager.saveTokens(access, refresh, meetInstance)
        // Auto-add the instance to saved Meet instances
        val instances = client.getMeetInstances().toMutableList()
        if (!instances.contains(meetInstance)) {
            instances.add(meetInstance)
            client.setMeetInstances(instances)
        }
        CoroutineScope(Dispatchers.IO).launch {
            try {
                client.setTokens(meetBaseUrl(meetInstance), access, refresh)
                val state = client.getSessionState()
                withContext(Dispatchers.Main) {
                    updateSessionFromState(state)
                }
            } catch (e: Exception) {
                Log.e("VisioManager", "Authentication failed", e)
                authManager.clearTokens()
            }
        }
    }

    fun logout() {
        CoroutineScope(Dispatchers.IO).launch {
            try {
                val instance =
                    authenticatedMeetInstance.ifEmpty {
                        client.getMeetInstances().firstOrNull() ?: ""
                    }
                if (instance.isNotEmpty()) {
                    client.logout(meetBaseUrl(instance))
                }
            } catch (_: Exception) {
                // No-op
            }
            authManager.clearTokens()
            withContext(Dispatchers.Main) {
                isAuthenticated = false
                authenticatedDisplayName = ""
                authenticatedEmail = ""
                authenticatedMeetInstance = ""
            }
        }
    }

    /**
     * Start Camera2 capture. Call after setCameraEnabled(true) succeeds
     * and CAMERA permission has been granted.
     */
    fun startCameraCapture() {
        // Stop any existing capture to avoid Camera2 state conflicts on reconnect
        stopCameraCapture()
        cameraCapture = CameraCapture(appContext).also { it.start() }
    }

    /**
     * Stop Camera2 capture. Call when camera is disabled or room disconnects.
     */
    fun stopCameraCapture() {
        cameraCapture?.stop()
        cameraCapture = null
    }

    /**
     * Start Camera2 capture in preview mode (blur + local render, no LiveKit).
     * Used for the pre-join lobby camera preview.
     */
    fun startPreviewCapture() {
        stopCameraCapture() // stop any existing capture

        // Sync BlurProcessor with persisted background mode so the preview
        // applies the user's last-selected filter immediately.
        try {
            val mode = client.getBackgroundMode()
            Log.i("VisioManager", "startPreviewCapture: syncing blur mode='$mode'")
            client.setBackgroundMode(mode)
        } catch (e: Exception) {
            Log.e("VisioManager", "startPreviewCapture: failed to sync blur mode", e)
        }

        cameraCapture =
            CameraCapture(appContext).also {
                it.previewMode = true
                it.start()
            }
    }

    /**
     * Stop preview capture and clear the local preview surface.
     * Only stops if the current capture is actually in preview mode,
     * to avoid killing a call capture that replaced it.
     */
    fun stopPreviewCapture() {
        val capture = cameraCapture ?: return
        if (!capture.previewMode) return // already replaced by call capture
        capture.stop()
        cameraCapture = null
        NativeVideo.nativeClearLocalPreviewSurface()
    }

    fun switchCamera(useFront: Boolean) {
        cameraCapture?.switchCamera(useFront)
    }

    fun isFrontCamera(): Boolean = cameraCapture?.isFront() ?: true

    /**
     * Start AudioRecord capture. Call after setMicrophoneEnabled(true) succeeds.
     */
    fun startAudioCapture(preferredDevice: AudioDeviceInfo? = null) {
        if (audioCapture != null) return
        val device =
            preferredDevice ?: run {
                val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
                am.getDevices(AudioManager.GET_DEVICES_INPUTS).firstOrNull { d ->
                    d.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                        d.type == AudioDeviceInfo.TYPE_BLE_HEADSET
                }
            }
        if (device != null) {
            Log.i("VisioManager", "Audio capture with device: ${device.productName}")
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
                am.setCommunicationDevice(device)
            }
        }
        audioCapture = AudioCapture().also { it.start(device) }
    }

    /**
     * Start synthetic audio capture (440Hz sine wave for E2E testing on emulators).
     */
    fun startSyntheticAudioCapture() {
        if (audioCapture != null) return
        audioCapture = AudioCapture().also { it.startSynthetic() }
    }

    /**
     * Start media file capture (audio + video from MP4) for E2E testing.
     * Replaces both synthetic audio and camera capture with decoded file content.
     */
    fun startMediaFileCapture(filePath: String) {
        if (mediaFileCapture != null) return
        mediaFileCapture =
            MediaFileCapture(filePath).also {
                it.startAudio()
                it.startVideo()
            }
    }

    /**
     * Stop media file capture.
     */
    fun stopMediaFileCapture() {
        mediaFileCapture?.let {
            it.stopAudio()
            it.stopVideo()
        }
        mediaFileCapture = null
    }

    /**
     * Stop AudioRecord capture. Call when mic is disabled or room disconnects.
     */
    fun stopAudioCapture() {
        audioCapture?.stop()
        audioCapture = null
    }

    /**
     * Start audio playout for remote participants. Call after connecting to room.
     * Acquires a partial wake lock so audio continues when screen is off.
     */
    fun startAudioPlayout() {
        if (audioPlayout != null) return
        // Set AudioManager to VoIP mode for low-latency audio routing
        val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        am.mode = AudioManager.MODE_IN_COMMUNICATION
        // Acquire partial wake lock to keep CPU active when screen is off
        val pm = appContext.getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock =
            pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "VisioMobile::AudioPlayout").apply {
                acquire(4 * 60 * 60 * 1000L) // 4-hour timeout as safety net
            }
        // Detect Bluetooth output device at startup
        val btOutput =
            am.getDevices(AudioManager.GET_DEVICES_OUTPUTS).firstOrNull { device ->
                device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                    device.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                    device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
            }
        if (btOutput != null) {
            Log.i("VisioManager", "Bluetooth output detected at startup: ${btOutput.productName}")
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                am.setCommunicationDevice(btOutput)
            }
        }
        audioPlayout = AudioPlayout().also { it.start(btOutput) }
    }

    /**
     * Stop audio playout. Call when disconnecting from room.
     */
    fun stopAudioPlayout() {
        audioPlayout?.stop()
        audioPlayout = null
        // Release wake lock
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
        // Release Bluetooth SCO channel so the car/headset regains audio
        val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            am.clearCommunicationDevice()
        } else {
            @Suppress("DEPRECATION")
            if (am.isBluetoothScoOn) {
                am.isBluetoothScoOn = false
                am.stopBluetoothSco()
            }
        }
        // Restore normal audio mode
        am.mode = AudioManager.MODE_NORMAL
    }

    /**
     * Start context detection for adaptive modes (network, motion, bluetooth).
     * Call after connecting to a room.
     */
    fun startContextDetection() {
        if (!client.isAdaptiveModeEnabled()) {
            Log.i("VisioManager", "Adaptive mode disabled, skipping context detection")
            return
        }
        Log.i("VisioManager", "Starting context detection")
        contextDetector = ContextDetector(appContext).also { it.start() }
    }

    fun stopContextDetection() {
        contextDetector?.stop()
        contextDetector = null
        _adaptiveMode.value = AdaptiveMode.OFFICE
        client.setAdaptiveModeOverride(AdaptiveMode.OFFICE)
    }

    /**
     * Route audio input to a specific device.
     */
    fun setAudioInputDevice(device: AudioDeviceInfo) {
        val capture = audioCapture ?: return // no-op if not capturing
        // Restart AudioRecord with new device to ensure routing takes effect
        capture.switchDevice(device)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            am.setCommunicationDevice(device)
        }
    }

    /**
     * Route audio output to a specific device.
     */
    fun setAudioOutputDevice(device: AudioDeviceInfo) {
        val playout = audioPlayout ?: return // no-op if not playing
        // Restart AudioTrack with new device to ensure routing takes effect
        playout.switchDevice(device)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            am.setCommunicationDevice(device)
        }
    }

    private fun routeAudioToBluetooth() {
        val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager

        // Save previous state for restore
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            previousAudioDevice = am.communicationDevice
        }

        // Ensure audio mode is set for communication (required for SCO)
        if (am.mode != AudioManager.MODE_IN_COMMUNICATION) {
            am.mode = AudioManager.MODE_IN_COMMUNICATION
            Log.i("VisioManager", "Set audio mode to MODE_IN_COMMUNICATION")
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            // Android 12+: use setCommunicationDevice
            val btDevice =
                am.availableCommunicationDevices.firstOrNull { device ->
                    device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                        device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
                }
            if (btDevice != null) {
                val success = am.setCommunicationDevice(btDevice)
                Log.i("VisioManager", "setCommunicationDevice(${btDevice.productName}): $success")
            } else {
                Log.w("VisioManager", "No Bluetooth device in availableCommunicationDevices")
                // Fallback: try startBluetoothSco
                @Suppress("DEPRECATION")
                am.startBluetoothSco()
                am.isBluetoothScoOn = true
                Log.i("VisioManager", "Started Bluetooth SCO (fallback)")
            }
        } else {
            // Pre-Android 12: use legacy SCO
            @Suppress("DEPRECATION")
            am.startBluetoothSco()
            am.isBluetoothScoOn = true
            Log.i("VisioManager", "Started Bluetooth SCO (legacy)")
        }

        // Restart audio tracks with Bluetooth device to ensure routing takes effect
        // (setPreferredDevice does not work after recording/playback has already started)
        val btOutput =
            am.getDevices(AudioManager.GET_DEVICES_OUTPUTS).firstOrNull { device ->
                device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                    device.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                    device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
            }
        val btInput =
            am.getDevices(AudioManager.GET_DEVICES_INPUTS).firstOrNull { device ->
                device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                    device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
            }
        btOutput?.let {
            if (audioPlayout != null) {
                setAudioOutputDevice(it)
                Log.i("VisioManager", "Restarted playout on Bluetooth output: ${it.productName}")
            }
        }
        btInput?.let {
            if (audioCapture != null) {
                setAudioInputDevice(it)
                Log.i("VisioManager", "Restarted capture on Bluetooth input: ${it.productName}")
            }
        }
    }

    private fun restoreDefaultAudioRoute() {
        val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            am.clearCommunicationDevice()
        } else {
            @Suppress("DEPRECATION")
            if (am.isBluetoothScoOn) {
                am.isBluetoothScoOn = false
                am.stopBluetoothSco()
            }
        }
        // Restart audio tracks without a device preference so they use default routing
        // (setPreferredDevice does not work after recording/playback has already started)
        audioCapture?.switchDevice(null)
        audioPlayout?.switchDevice(null)
        previousAudioDevice = null
        Log.i("VisioManager", "Restored default audio routing (tracks restarted)")
    }

    /**
     * Start monitoring audio focus changes to detect phone calls.
     * When a phone call starts, Android requests audio focus and we lose ours.
     * Also registers an AudioDeviceCallback to fall back to default audio when
     * a Bluetooth device disconnects mid-session.
     */
    private fun startAudioFocusMonitoring() {
        val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        registerBluetoothDisconnectCallback(am)
        createAudioFocusListener()
        val result = requestAudioFocus(am)
        Log.i("VisioManager", "Audio focus requested: result=$result")
    }

    private fun registerBluetoothDisconnectCallback(am: AudioManager) {
        bluetoothDeviceCallback =
            object : AudioDeviceCallback() {
                override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) {
                    val btRemoved =
                        removedDevices.any { device ->
                            isBluetoothDevice(device)
                        }
                    if (btRemoved && _connectionState.value is ConnectionState.Connected) {
                        Log.i("VisioManager", "Bluetooth audio device removed mid-session — falling back to system default")
                        scope.launch(Dispatchers.IO) { restoreDefaultAudioRoute() }
                    }
                }
            }
        am.registerAudioDeviceCallback(bluetoothDeviceCallback, null)
    }

    private fun isBluetoothDevice(device: AudioDeviceInfo): Boolean =
        device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
            device.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
            device.type == AudioDeviceInfo.TYPE_BLE_HEADSET

    private fun createAudioFocusListener() {
        audioFocusListener =
            AudioManager.OnAudioFocusChangeListener { focusChange ->
                handleAudioFocusChange(focusChange)
            }
    }

    private fun handleAudioFocusChange(focusChange: Int) {
        when (focusChange) {
            AudioManager.AUDIOFOCUS_LOSS,
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT,
            -> {
                Log.i("VisioManager", "Audio focus lost (phone call?) — pausing audio")
                wasPlayingBeforeFocusLoss = audioPlayout != null
                stopAudioPlayout()
                stopAudioCapture()
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                Log.i("VisioManager", "Audio focus regained — resuming audio")
                if (wasPlayingBeforeFocusLoss && connectionState.value is ConnectionState.Connected) {
                    scope.launch(Dispatchers.IO) {
                        startAudioPlayout()
                        if (client.isMicrophoneEnabled()) startAudioCapture()
                    }
                }
                wasPlayingBeforeFocusLoss = false
            }
        }
    }

    private fun requestAudioFocus(am: AudioManager): Int =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val request =
                android.media.AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                    .setAudioAttributes(
                        android.media.AudioAttributes.Builder()
                            .setUsage(android.media.AudioAttributes.USAGE_VOICE_COMMUNICATION)
                            .setContentType(android.media.AudioAttributes.CONTENT_TYPE_SPEECH)
                            .build(),
                    )
                    .setOnAudioFocusChangeListener(audioFocusListener!!)
                    .build()
            am.requestAudioFocus(request)
        } else {
            @Suppress("DEPRECATION")
            am.requestAudioFocus(audioFocusListener, AudioManager.STREAM_VOICE_CALL, AudioManager.AUDIOFOCUS_GAIN)
        }

    /**
     * Stop monitoring audio focus changes. Call when disconnecting.
     */
    private fun stopAudioFocusMonitoring() {
        val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager

        // Unregister Bluetooth disconnect fallback callback
        bluetoothDeviceCallback?.let { am.unregisterAudioDeviceCallback(it) }
        bluetoothDeviceCallback = null

        audioFocusListener?.let { listener ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val request =
                    android.media.AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                        .setOnAudioFocusChangeListener(listener)
                        .build()
                am.abandonAudioFocusRequest(request)
            } else {
                @Suppress("DEPRECATION")
                am.abandonAudioFocus(listener)
            }
        }
        audioFocusListener = null
        wasPlayingBeforeFocusLoss = false
    }

    /**
     * Called by ContextDetector when a Bluetooth audio device connects.
     * Auto-routes audio if we're in an active call.
     */
    fun onBluetoothAudioDeviceConnected() {
        if (_connectionState.value !is ConnectionState.Connected) return
        Log.i("VisioManager", "Auto-routing audio to newly connected Bluetooth device")
        scope.launch(Dispatchers.IO) {
            routeAudioToBluetooth()
        }
    }

    /**
     * Called by ContextDetector when a Bluetooth audio device disconnects.
     * Restores default routing if no other Bluetooth devices remain.
     */
    fun onBluetoothAudioDeviceDisconnected() {
        if (_connectionState.value !is ConnectionState.Connected) return
        val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        val remainingBtDevice =
            am.getDevices(AudioManager.GET_DEVICES_OUTPUTS).firstOrNull { device ->
                device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                    device.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                    device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
            }
        if (remainingBtDevice != null) {
            // Another BT device still connected — route to it
            Log.i("VisioManager", "Switching audio to remaining BT device: ${remainingBtDevice.productName}")
            scope.launch(Dispatchers.IO) {
                routeAudioToBluetooth()
            }
        } else {
            // No BT devices left — restore to phone speaker/mic
            Log.i("VisioManager", "No more Bluetooth audio devices, restoring phone speaker/mic")
            scope.launch(Dispatchers.IO) {
                restoreDefaultAudioRoute()
            }
        }
    }

    /**
     * Admit a waiting participant into the room (host action).
     */
    fun admitParticipant(participantId: String) {
        scope.launch {
            try {
                client.admitParticipant(participantId)
                _waitingParticipants.value = _waitingParticipants.value.filter { it.id != participantId }
            } catch (e: Exception) {
                Log.e("VisioManager", "admit failed: ${e.message}")
            }
        }
    }

    /**
     * Deny a waiting participant entry (host action).
     */
    fun denyParticipant(participantId: String) {
        scope.launch {
            try {
                client.denyParticipant(participantId)
                _waitingParticipants.value = _waitingParticipants.value.filter { it.id != participantId }
            } catch (e: Exception) {
                Log.e("VisioManager", "deny failed: ${e.message}")
            }
        }
    }

    /**
     * Clear the lobby join notification banner.
     */
    fun clearLobbyNotification() {
        _lobbyNotification.value = null
    }

    /**
     * Cancel waiting in the lobby and disconnect.
     */
    fun cancelLobby() {
        client.cancelLobby()
    }

    fun setCurrentRoom(
        roomId: String?,
        accessLevel: String,
    ) {
        currentRoomId = roomId
        _currentAccessLevel = accessLevel
    }

    fun refreshAccesses() {
        val roomId = currentRoomId ?: return
        scope.launch {
            try {
                val accesses = client.listAccesses(roomId)
                _roomAccesses.value = accesses
            } catch (_: Exception) {
                // No-op
            }
        }
    }

    fun addAccessMember(
        userId: String,
        onDone: () -> Unit = {},
    ) {
        val roomId = currentRoomId ?: return
        scope.launch {
            try {
                client.addAccess(userId, roomId)
                refreshAccesses()
            } catch (_: Exception) {
                // No-op
            }
            withContext(Dispatchers.Main) { onDone() }
        }
    }

    fun removeAccessMember(accessId: String) {
        scope.launch {
            try {
                client.removeAccess(accessId)
                refreshAccesses()
            } catch (_: Exception) {
                // No-op
            }
        }
    }

    fun sendReaction(emoji: String) {
        scope.launch {
            try {
                client.sendReaction(emoji)
                // Show reaction locally (server echo is filtered out in Rust)
                val reaction =
                    ReactionData(
                        id = reactionIdCounter++,
                        participantSid = "local",
                        participantName = client.getSettings().displayName ?: "",
                        emoji = emoji,
                        timestamp = System.currentTimeMillis(),
                    )
                _reactions.value = _reactions.value + reaction
            } catch (e: Exception) {
                Log.e("VISIO", "sendReaction failed: ${e.message}")
            }
        }
    }

    /**
     * Trigger an immediate calendar refresh (manual or on-tab-switch).
     */
    fun refreshCalendarNow() {
        _calendarLoading.value = true
        scope.launch(Dispatchers.IO) {
            try {
                client.refreshCalendarNow()
            } catch (e: Exception) {
                Log.e("VisioManager", "Calendar refresh failed", e)
                _calendarLoading.value = false
            }
        }
    }

    /**
     * Send a local notification for an upcoming meeting.
     * @param meeting the meeting to notify about
     * @param type "imminent" (15 min), "soon" (5 min), or "started"
     */
    fun sendMeetingNotification(
        meeting: Meeting,
        type: String,
    ) {
        val nm =
            appContext.getSystemService(Context.NOTIFICATION_SERVICE)
                as android.app.NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            !nm.areNotificationsEnabled()
        ) {
            return
        }
        val contentText =
            when (type) {
                "imminent" -> "Dans 15 min"
                "soon" -> "Dans 5 min"
                else -> "Commence maintenant"
            }
        val deepLinkIntent =
            android.content.Intent(appContext, MainActivity::class.java).apply {
                action = android.content.Intent.ACTION_VIEW
                data = android.net.Uri.parse(meeting.deepLink)
                flags = android.content.Intent.FLAG_ACTIVITY_SINGLE_TOP or android.content.Intent.FLAG_ACTIVITY_CLEAR_TOP
            }
        val pendingIntent =
            android.app.PendingIntent.getActivity(
                appContext,
                meeting.id.hashCode(),
                deepLinkIntent,
                android.app.PendingIntent.FLAG_UPDATE_CURRENT or android.app.PendingIntent.FLAG_IMMUTABLE,
            )
        val notification =
            NotificationCompat.Builder(appContext, MEETING_CHANNEL_ID)
                .setSmallIcon(android.R.drawable.ic_popup_reminder)
                .setContentTitle(meeting.summary)
                .setContentText(contentText)
                .setPriority(NotificationCompat.PRIORITY_HIGH)
                .setAutoCancel(true)
                .setContentIntent(pendingIntent)
                .build()
        nm.notify(meeting.id.hashCode(), notification)
    }

    /**
     * Full teardown: stop captures, playout, cancel pending coroutines, disconnect.
     */
    fun disconnect() {
        callScreenInitialized = false
        stopAudioFocusMonitoring()
        contextDetector?.stop()
        contextDetector = null
        stopCameraCapture()
        stopAudioCapture()
        stopAudioPlayout()
        scope.cancel()
        scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        client.disconnect()
    }

    fun refreshParticipantsPublic() = refreshParticipants()

    private fun refreshParticipants() {
        scope.launch {
            val list = client.participants()
            list.forEach { p ->
                if (p.hasVideo) {
                    Log.d("VISIO", "Participant ${p.sid} (${p.name}): hasVideo=true trackSid=${p.videoTrackSid}")
                }
            }
            _participants.value = list
        }
    }

    private fun refreshChatMessages() {
        scope.launch { _chatMessages.value = client.chatMessages() }
    }

    /**
     * Called when the app goes to background. Stop camera to save battery
     * but keep audio active (wake lock protects CPU).
     */
    fun onAppBackgrounded() {
        // Don't stop camera here — permission dialogs and system overlays
        // trigger onStop while the call is still active. The camera will be
        // properly stopped when the user disconnects (via disconnect()).
        // This also allows the camera to keep sending frames during brief
        // background transitions (e.g. notification shade pull-down).
    }

    /**
     * Called when the app returns to foreground. Restart camera if it was
     * enabled, and trigger reconnection if the connection was lost.
     */
    fun onAppForegrounded() {
        scope.launch {
            when (connectionState.value) {
                is ConnectionState.Connected -> {
                    if (client.isCameraEnabled()) {
                        startCameraCapture()
                    }
                    refreshParticipantsPublic()
                }
                is ConnectionState.Disconnected -> {
                    try {
                        client.reconnect()
                    } catch (e: Exception) {
                        Log.e("VISIO", "Foreground reconnection failed: ${e.message}")
                    }
                }
                else -> {
                    // No-op
                }
            }
        }
    }

    override fun onEvent(event: VisioEvent) {
        when (event) {
            is VisioEvent.ConnectionStateChanged -> {
                _connectionState.value = event.state
                handleConnectionStateChanged(event.state)
            }
            is VisioEvent.ParticipantJoined -> {
                refreshParticipants()
            }
            is VisioEvent.ParticipantLeft -> {
                refreshParticipants()
                // Remove from hand raised map
                val sid = event.participantSid
                _handRaisedMap.value = _handRaisedMap.value.minus(sid)
            }
            is VisioEvent.TrackMuted -> {
                refreshParticipants()
            }
            is VisioEvent.TrackUnmuted -> {
                refreshParticipants()
            }
            is VisioEvent.ActiveSpeakersChanged -> {
                _activeSpeakers.value = event.participantSids
            }
            is VisioEvent.ConnectionQualityChanged -> {
                refreshParticipants()
            }
            is VisioEvent.ChatMessageReceived -> {
                refreshChatMessages()
            }
            is VisioEvent.HandRaisedChanged -> {
                val sid = event.participantSid
                val raised = event.raised
                val position = event.position.toInt()
                if (raised) {
                    _handRaisedMap.value = _handRaisedMap.value.plus(sid to position)
                } else {
                    _handRaisedMap.value = _handRaisedMap.value.minus(sid)
                }
                // Update local hand state — check if this is local participant
                scope.launch {
                    _isHandRaised.value = client.isHandRaised()
                }
            }
            is VisioEvent.UnreadCountChanged -> {
                _unreadCount.value = event.count.toInt()
            }
            is VisioEvent.TrackSubscribed -> {
                val info = event.info
                Log.d(
                    "VISIO",
                    "TrackSubscribed: participant=${info.participantSid} kind=${info.kind} source=${info.source} trackSid=${info.sid}",
                )
                if (info.source == TrackSource.SCREEN_SHARE && info.kind == TrackKind.VIDEO) {
                    _screenShareSubscribed.value = info.participantSid
                }
                refreshParticipants()
            }
            is VisioEvent.TrackUnsubscribed -> {
                Log.d("VISIO", "TrackUnsubscribed: trackSid=${event.trackSid}")
                refreshParticipants()
            }
            is VisioEvent.LobbyParticipantJoined -> {
                val participant = WaitingParticipant(event.id, event.username)
                val current = _waitingParticipants.value.toMutableList()
                if (current.none { it.id == event.id }) {
                    current.add(participant)
                    _waitingParticipants.value = current
                }
                _lobbyNotification.value = participant
            }
            is VisioEvent.LobbyParticipantLeft -> {
                _waitingParticipants.value = _waitingParticipants.value.filter { it.id != event.id }
            }
            is VisioEvent.LobbyDenied -> {
                _lobbyDenied.value = true
            }
            is VisioEvent.ReactionReceived -> {
                val reaction =
                    ReactionData(
                        id = reactionIdCounter++,
                        participantSid = event.participantSid,
                        participantName = event.participantName,
                        emoji = event.emoji,
                        timestamp = System.currentTimeMillis(),
                    )
                _reactions.value = _reactions.value + reaction
            }
            is VisioEvent.ConnectionLost -> {
                scope.launch {
                    try {
                        client.reconnect()
                    } catch (e: Exception) {
                        Log.e("VISIO", "Auto-reconnection failed: ${e.message}")
                    }
                }
            }
            is VisioEvent.BandwidthModeChanged -> {
                Log.d("VISIO", "Bandwidth mode changed: ${event.mode}")
                _bandwidthMode.value = event.mode
            }
            is VisioEvent.AloneInRoom -> {
                Log.d("VISIO", "Alone in room")
            }
            is VisioEvent.AloneInRoomCancelled -> {
                Log.d("VISIO", "No longer alone in room")
            }
            is VisioEvent.DisconnectedByAdmin -> {
                Log.i("VISIO", "Disconnected by admin")
            }
            is VisioEvent.DisconnectedDuplicateIdentity -> {
                Log.i("VISIO", "Disconnected: duplicate identity")
            }
            is VisioEvent.LobbyTimeout -> {
                Log.d("VISIO", "Lobby timeout")
            }
            is VisioEvent.MuteRequested -> {
                Log.d("VISIO", "Mute requested")
                scope.launch(Dispatchers.IO) {
                    try {
                        client.setMicrophoneEnabled(false)
                    } catch (e: Exception) {
                        Log.e("VISIO", "Failed to handle mute request", e)
                    }
                }
            }
            is VisioEvent.MeetingsUpdated -> {
                val prev = _upcomingMeetings.value
                _upcomingMeetings.value = event.meetings
                _calendarLoading.value = false
                // Only show sync toast when meetings actually changed
                val prevIds = prev.map { it.id }.toSet()
                val newIds = event.meetings.map { it.id }.toSet()
                if (prevIds != newIds) {
                    _calendarSyncResult.value =
                        CalendarSyncResult.Success(event.meetings.size)
                }
            }
            is VisioEvent.MeetingImminent -> {
                sendMeetingNotification(event.meeting, "imminent")
            }
            is VisioEvent.MeetingStartingSoon -> {
                sendMeetingNotification(event.meeting, "soon")
            }
            is VisioEvent.MeetingStarted -> {
                sendMeetingNotification(event.meeting, "started")
            }
            is VisioEvent.CalendarError -> {
                Log.e("VisioManager", "Calendar error: ${event.message}")
                _calendarLoading.value = false
                _calendarSyncResult.value = CalendarSyncResult.Error(event.message)
            }
            is VisioEvent.AdaptiveModeChanged -> {
                handleAdaptiveModeChanged(event.mode)
            }
            is VisioEvent.ParticipantOrderChanged -> {
                Log.d("VISIO", "Participant order changed")
            }
            is VisioEvent.PageChanged -> {
                Log.d("VISIO", "Page changed: ${event.page}")
            }
            is VisioEvent.MainParticipantChanged -> {
                Log.d("VISIO", "Main participant changed")
            }
            is VisioEvent.LayoutModeChanged -> {
                Log.d("VISIO", "Layout mode changed")
            }
        }
    }

    private fun handleConnectionStateChanged(state: ConnectionState) {
        when (state) {
            is ConnectionState.Connected -> {
                connectionTimestampMs = System.currentTimeMillis()
                refreshParticipants()
                refreshChatMessages()
                CallForegroundService.start(appContext)
                android.os.Handler(android.os.Looper.getMainLooper()).post {
                    startContextDetection()
                }
                startAudioFocusMonitoring()
                // Republish camera track and restart capture on reconnection
                scope.launch {
                    if (client.isCameraEnabled()) {
                        try {
                            client.setCameraEnabled(true)
                        } catch (e: Exception) {
                            Log.e("VISIO", "Failed to republish camera on reconnect: ${e.message}")
                        }
                        startCameraCapture()
                    }
                }
            }
            is ConnectionState.Disconnected -> {
                _handRaisedMap.value = emptyMap()
                _unreadCount.value = 0
                _isHandRaised.value = false
                _waitingParticipants.value = emptyList()
                _lobbyNotification.value = null
                _bandwidthMode.value = uniffi.visio.BandwidthMode.FULL
                CallForegroundService.stop(appContext)
            }
            else -> {
                // No-op
            }
        }
    }

    private fun handleAdaptiveModeChanged(newMode: uniffi.visio.AdaptiveMode) {
        val previousMode = _adaptiveMode.value
        _adaptiveMode.value = newMode
        Log.d("VISIO", "Adaptive mode changed: $previousMode -> $newMode")
        if (newMode == uniffi.visio.AdaptiveMode.CAR) {
            scope.launch(Dispatchers.IO) { enterCarMode() }
        } else if (previousMode == uniffi.visio.AdaptiveMode.CAR) {
            scope.launch(Dispatchers.IO) { exitCarMode() }
        }
    }

    private suspend fun enterCarMode() {
        awaitConnectionGracePeriod()
        cameraWasEnabledBeforeCar = client.isCameraEnabled()
        if (cameraWasEnabledBeforeCar) {
            stopCameraCapture()
            client.setCameraEnabled(false)
        }
        kotlinx.coroutines.delay(200)
        routeAudioToBluetooth()
        logCarModeBluetoothState()
    }

    private suspend fun awaitConnectionGracePeriod() {
        val elapsed = System.currentTimeMillis() - connectionTimestampMs
        if (elapsed < CONNECTION_GRACE_MS) {
            val delayMs = CONNECTION_GRACE_MS - elapsed
            Log.d("VISIO", "CAR mode: waiting ${delayMs}ms for connection grace period")
            kotlinx.coroutines.delay(delayMs)
        }
    }

    private fun logCarModeBluetoothState() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val am = appContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            val commDevice = am.communicationDevice
            Log.i("VisioManager", "CAR mode: communication device after routing = ${commDevice?.productName ?: "none"}")
        }
    }

    private suspend fun exitCarMode() {
        restoreDefaultAudioRoute()
        if (cameraWasEnabledBeforeCar) {
            try {
                client.setCameraEnabled(true)
                startCameraCapture()
            } catch (e: Exception) {
                Log.e("VISIO", "Failed to restore camera after car mode", e)
            }
            cameraWasEnabledBeforeCar = false
        }
    }
}

data class ReactionData(
    val id: Long,
    val participantSid: String,
    val participantName: String,
    val emoji: String,
    val timestamp: Long,
)
