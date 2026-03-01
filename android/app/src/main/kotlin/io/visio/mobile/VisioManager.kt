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

    private val _connectionState = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)
    val connectionState: StateFlow<ConnectionState> = _connectionState.asStateFlow()

    private val _participants = MutableStateFlow<List<ParticipantInfo>>(emptyList())
    val participants: StateFlow<List<ParticipantInfo>> = _participants.asStateFlow()

    private val _chatMessages = MutableStateFlow<List<ChatMessage>>(emptyList())
    val chatMessages: StateFlow<List<ChatMessage>> = _chatMessages.asStateFlow()

    private val _activeSpeakers = MutableStateFlow<List<String>>(emptyList())
    val activeSpeakers: StateFlow<List<String>> = _activeSpeakers.asStateFlow()

    private var initialized = false

    fun initialize(context: Context) {
        if (initialized) return
        val dataDir = context.filesDir.absolutePath
        _client = VisioClient(dataDir)
        _client.addListener(this)
        initialized = true
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
            }
            is VisioEvent.ParticipantJoined -> {
                refreshParticipants()
            }
            is VisioEvent.ParticipantLeft -> {
                refreshParticipants()
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
            is VisioEvent.TrackSubscribed,
            is VisioEvent.TrackUnsubscribed -> {
                // No-op for now; video rendering handled separately
            }
        }
    }
}
