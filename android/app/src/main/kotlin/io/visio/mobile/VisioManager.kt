package io.visio.mobile

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.visio.ChatMessage
import uniffi.visio.ConnectionState
import uniffi.visio.ParticipantInfo
import uniffi.visio.VisioClient
import uniffi.visio.VisioEvent
import uniffi.visio.VisioEventListener

object VisioManager : VisioEventListener {

    // Library loaded and WebRTC initialized by VisioApplication.onCreate()
    private lateinit var _client: VisioClient
    val client: VisioClient get() = _client

    // IO scope for callbacks that call back into Rust (avoids nested block_on)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    // Camera capture (Camera2 -> JNI -> NativeVideoSource)
    private var cameraCapture: CameraCapture? = null
    // Audio capture (AudioRecord -> JNI -> NativeAudioSource)
    private var audioCapture: AudioCapture? = null
    // Audio playout (Rust playout buffer -> JNI -> AudioTrack)
    private var audioPlayout: AudioPlayout? = null
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

    private var initialized = false

    fun initialize(context: Context) {
        if (initialized) return
        appContext = context.applicationContext
        val dataDir = context.filesDir.absolutePath
        _client = VisioClient(dataDir)
        _client.addListener(this)
        initialized = true
    }

    /**
     * Start Camera2 capture. Call after setCameraEnabled(true) succeeds
     * and CAMERA permission has been granted.
     */
    fun startCameraCapture() {
        if (cameraCapture != null) return
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
     * Start AudioRecord capture. Call after setMicrophoneEnabled(true) succeeds.
     */
    fun startAudioCapture() {
        if (audioCapture != null) return
        audioCapture = AudioCapture().also { it.start() }
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
     */
    fun startAudioPlayout() {
        if (audioPlayout != null) return
        audioPlayout = AudioPlayout().also { it.start() }
    }

    /**
     * Stop audio playout. Call when disconnecting from room.
     */
    fun stopAudioPlayout() {
        audioPlayout?.stop()
        audioPlayout = null
    }

    private fun refreshParticipants() {
        scope.launch { _participants.value = client.participants() }
    }

    private fun refreshChatMessages() {
        scope.launch { _chatMessages.value = client.chatMessages() }
    }

    override fun onEvent(event: VisioEvent) {
        when (event) {
            is VisioEvent.ConnectionStateChanged -> {
                _connectionState.value = event.state
                if (event.state is ConnectionState.Connected) {
                    refreshParticipants()
                    refreshChatMessages()
                }
                if (event.state is ConnectionState.Disconnected) {
                    // Reset state on disconnect
                    _handRaisedMap.value = emptyMap()
                    _unreadCount.value = 0
                    _isHandRaised.value = false
                }
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
                _unreadCount.value = event.v1.toInt()
            }
            is VisioEvent.TrackSubscribed,
            is VisioEvent.TrackUnsubscribed -> {
                // No-op for now; video rendering handled separately
            }
        }
    }
}
