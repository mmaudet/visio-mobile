package io.visio.mobile.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Call
import androidx.compose.material.icons.filled.CallEnd
import androidx.compose.material.icons.filled.Chat
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.MicOff
import androidx.compose.material.icons.filled.Videocam
import androidx.compose.material.icons.filled.VideocamOff
import androidx.compose.material3.BottomAppBar
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import io.visio.mobile.VisioManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.visio.ConnectionState

@Composable
fun CallScreen(
    roomUrl: String,
    username: String,
    onNavigateToChat: () -> Unit,
    onHangUp: () -> Unit
) {
    val connectionState by VisioManager.connectionState.collectAsState()
    val participants by VisioManager.participants.collectAsState()

    val context = LocalContext.current
    var micEnabled by remember { mutableStateOf(true) }
    var cameraEnabled by remember { mutableStateOf(true) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    // Camera permission launcher
    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            try {
                VisioManager.client.setCameraEnabled(true)
                VisioManager.startCameraCapture()
                cameraEnabled = true
            } catch (_: Exception) {}
        }
    }

    // Stop camera capture when leaving the call screen
    DisposableEffect(Unit) {
        onDispose {
            VisioManager.stopCameraCapture()
        }
    }

    // Use Unit key so this only fires once per CallScreen composition,
    // not on every back-navigation from ChatScreen.
    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            try {
                // Only connect if not already connected (prevents double sessions)
                val state = VisioManager.connectionState.value
                if (state is ConnectionState.Connected || state is ConnectionState.Connecting) {
                    micEnabled = VisioManager.client.isMicrophoneEnabled()
                    cameraEnabled = VisioManager.client.isCameraEnabled()
                    return@withContext
                }
                val user = username.ifBlank { null }
                VisioManager.client.connect(roomUrl, user)
                micEnabled = VisioManager.client.isMicrophoneEnabled()
                cameraEnabled = VisioManager.client.isCameraEnabled()
            } catch (e: Exception) {
                errorMessage = e.message ?: "Connection failed"
            }
        }
    }

    Scaffold(
        bottomBar = {
            BottomAppBar {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceEvenly,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Mic toggle
                    IconButton(onClick = {
                        try {
                            val newState = !micEnabled
                            VisioManager.client.setMicrophoneEnabled(newState)
                            micEnabled = newState
                        } catch (_: Exception) {}
                    }) {
                        Icon(
                            imageVector = if (micEnabled) Icons.Default.Mic else Icons.Default.MicOff,
                            contentDescription = if (micEnabled) "Mute microphone" else "Unmute microphone"
                        )
                    }

                    // Camera toggle
                    IconButton(onClick = {
                        try {
                            val newState = !cameraEnabled
                            if (newState) {
                                // Check / request CAMERA permission before enabling
                                val hasPermission = ContextCompat.checkSelfPermission(
                                    context, Manifest.permission.CAMERA
                                ) == PackageManager.PERMISSION_GRANTED

                                if (hasPermission) {
                                    VisioManager.client.setCameraEnabled(true)
                                    VisioManager.startCameraCapture()
                                    cameraEnabled = true
                                } else {
                                    cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
                                }
                            } else {
                                VisioManager.stopCameraCapture()
                                VisioManager.client.setCameraEnabled(false)
                                cameraEnabled = false
                            }
                        } catch (_: Exception) {}
                    }) {
                        Icon(
                            imageVector = if (cameraEnabled) Icons.Default.Videocam else Icons.Default.VideocamOff,
                            contentDescription = if (cameraEnabled) "Disable camera" else "Enable camera"
                        )
                    }

                    // Chat button
                    IconButton(onClick = onNavigateToChat) {
                        Icon(
                            imageVector = Icons.Default.Chat,
                            contentDescription = "Open chat"
                        )
                    }

                    // Hang up
                    FilledIconButton(
                        onClick = {
                            VisioManager.stopCameraCapture()
                            VisioManager.client.disconnect()
                            onHangUp()
                        },
                        colors = IconButtonDefaults.filledIconButtonColors(
                            containerColor = MaterialTheme.colorScheme.error
                        )
                    ) {
                        Icon(
                            imageVector = Icons.Default.CallEnd,
                            contentDescription = "Hang up",
                            tint = MaterialTheme.colorScheme.onError
                        )
                    }
                }
            }
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(16.dp)
        ) {
            // Connection state banner
            ConnectionStateBanner(connectionState, errorMessage)

            Spacer(modifier = Modifier.height(16.dp))

            Text(
                text = "Participants (${participants.size})",
                style = MaterialTheme.typography.titleMedium
            )

            Spacer(modifier = Modifier.height(8.dp))

            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                items(participants, key = { it.sid }) { participant ->
                    ParticipantCard(participant)
                }
            }
        }
    }
}

@Composable
private fun ConnectionStateBanner(state: ConnectionState, errorMessage: String?) {
    when {
        errorMessage != null -> {
            Card(
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(
                    text = "Error: $errorMessage",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(12.dp),
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }
        state is ConnectionState.Connecting -> {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                CircularProgressIndicator(modifier = Modifier.height(20.dp))
                Text("Connecting...", style = MaterialTheme.typography.bodyMedium)
            }
        }
        state is ConnectionState.Reconnecting -> {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                CircularProgressIndicator(modifier = Modifier.height(20.dp))
                Text(
                    "Reconnecting (attempt ${state.attempt})...",
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }
        state is ConnectionState.Connected -> {
            Text(
                "Connected",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.primary
            )
        }
        state is ConnectionState.Disconnected -> {
            Text(
                "Disconnected",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.outline
            )
        }
    }
}

@Composable
private fun ParticipantCard(participant: uniffi.visio.ParticipantInfo) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Column {
                Text(
                    text = participant.name ?: participant.identity,
                    style = MaterialTheme.typography.bodyLarge
                )
                if (participant.name != null) {
                    Text(
                        text = participant.identity,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.outline
                    )
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                Icon(
                    imageVector = if (participant.isMuted) Icons.Default.MicOff else Icons.Default.Mic,
                    contentDescription = null,
                    tint = if (participant.isMuted) MaterialTheme.colorScheme.outline
                           else MaterialTheme.colorScheme.primary
                )
                Icon(
                    imageVector = if (participant.hasVideo) Icons.Default.Videocam else Icons.Default.VideocamOff,
                    contentDescription = null,
                    tint = if (participant.hasVideo) MaterialTheme.colorScheme.primary
                           else MaterialTheme.colorScheme.outline
                )
            }
        }
    }
}
