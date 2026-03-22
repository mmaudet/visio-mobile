package io.visio.mobile.ui

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.graphics.SurfaceTexture
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.media.AudioDeviceInfo
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Handler
import android.os.HandlerThread
import android.view.Surface
import android.view.TextureView
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
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
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.VolumeUp
import androidx.compose.material.icons.filled.Cameraswitch
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Videocam
import androidx.compose.material.icons.filled.VideocamOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.visio.ConnectionState
import kotlin.math.sqrt

private const val KEY_DEVICE_MICROPHONE = "device.microphone"
private const val KEY_SETTINGS_BACKGROUND = "settings.incall.background"

// ── Waiting-room state ────────────────────────────────────────────────────────

sealed interface WaitingState {
    data object Idle : WaitingState

    data object Waiting : WaitingState

    data object Denied : WaitingState

    data object Timeout : WaitingState
}

// ── Local camera preview (Camera2, TextureView with center-crop) ─────────────

class LocalCameraPreview(
    context: Context,
    private var useFront: Boolean,
) : TextureView(context),
    TextureView.SurfaceTextureListener {
    private var session: CameraCaptureSession? = null
    private var camera: CameraDevice? = null
    private val handlerThread = HandlerThread("PreviewCamera").also { it.start() }
    private val handler = Handler(handlerThread.looper)

    init {
        surfaceTextureListener = this
    }

    override fun onSurfaceTextureAvailable(
        surface: SurfaceTexture,
        width: Int,
        height: Int,
    ) {
        openCamera()
    }

    override fun onSurfaceTextureSizeChanged(
        surface: SurfaceTexture,
        width: Int,
        height: Int,
    ) {
        updateTransform()
    }

    override fun onSurfaceTextureDestroyed(surface: SurfaceTexture): Boolean {
        stopCamera()
        handlerThread.quitSafely()
        return true
    }

    override fun onSurfaceTextureUpdated(surface: SurfaceTexture) {
        // No-op: frame updates handled by camera capture session
    }

    fun switchCamera(front: Boolean) {
        if (front == useFront) return
        useFront = front
        stopCamera()
        openCamera()
    }

    @SuppressLint("MissingPermission")
    private fun openCamera() {
        val texture = surfaceTexture ?: return
        val surface = Surface(texture)
        val cm = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        val cameraId =
            cm.cameraIdList.firstOrNull { id ->
                val facing = cm.getCameraCharacteristics(id).get(CameraCharacteristics.LENS_FACING)
                if (useFront) {
                    facing == CameraCharacteristics.LENS_FACING_FRONT
                } else {
                    facing == CameraCharacteristics.LENS_FACING_BACK
                }
            } ?: return

        cm.openCamera(
            cameraId,
            object : CameraDevice.StateCallback() {
                override fun onOpened(dev: CameraDevice) {
                    camera = dev
                    @Suppress("DEPRECATION")
                    dev.createCaptureSession(
                        listOf(surface),
                        object : CameraCaptureSession.StateCallback() {
                            override fun onConfigured(sess: CameraCaptureSession) {
                                session = sess
                                val req =
                                    dev
                                        .createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW)
                                        .apply { addTarget(surface) }
                                        .build()
                                sess.setRepeatingRequest(req, null, handler)
                                post { updateTransform() }
                            }

                            override fun onConfigureFailed(sess: CameraCaptureSession) {
                                // No-op: preview failure is non-fatal, user can still join
                            }
                        },
                        handler,
                    )
                }

                override fun onDisconnected(dev: CameraDevice) {
                    dev.close()
                }

                override fun onError(
                    dev: CameraDevice,
                    error: Int,
                ) {
                    dev.close()
                }
            },
            handler,
        )
    }

    private fun updateTransform() {
        val viewWidth = width.toFloat()
        val viewHeight = height.toFloat()
        if (viewWidth == 0f || viewHeight == 0f) return

        val cm = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        val cameraId =
            cm.cameraIdList.firstOrNull { id ->
                val facing = cm.getCameraCharacteristics(id).get(CameraCharacteristics.LENS_FACING)
                if (useFront) {
                    facing == CameraCharacteristics.LENS_FACING_FRONT
                } else {
                    facing == CameraCharacteristics.LENS_FACING_BACK
                }
            } ?: return

        val characteristics = cm.getCameraCharacteristics(cameraId)
        val map = characteristics.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP) ?: return
        val previewSize =
            map.getOutputSizes(SurfaceTexture::class.java)
                ?.maxByOrNull { it.width * it.height } ?: return

        // Camera sensor is rotated 90 degrees relative to the view on most devices
        val previewWidth = previewSize.height.toFloat()
        val previewHeight = previewSize.width.toFloat()

        val scaleX = viewWidth / previewWidth
        val scaleY = viewHeight / previewHeight
        val scale = maxOf(scaleX, scaleY) // center-crop

        val matrix = Matrix()
        matrix.setScale(
            scale * previewWidth / viewWidth,
            scale * previewHeight / viewHeight,
            viewWidth / 2f,
            viewHeight / 2f,
        )
        setTransform(matrix)
    }

    private fun stopCamera() {
        session?.close()
        session = null
        camera?.close()
        camera = null
    }
}

// ── PreJoinScreen ─────────────────────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PreJoinScreen(
    roomUrl: String,
    initialUsername: String,
    onJoin: (finalUsername: String) -> Unit,
    onCancel: () -> Unit,
) {
    val context = LocalContext.current
    val lang = VisioManager.currentLang
    val isDark = VisioManager.currentTheme == "dark"
    val coroutineScope = rememberCoroutineScope()

    // ── Display name ─────────────────────────────────────────────────────────
    var displayName by remember { mutableStateOf(initialUsername.ifBlank { VisioManager.displayName }) }

    // ── Room slug (for display) ───────────────────────────────────────────────
    val roomSlug =
        remember(roomUrl) {
            if ('/' in roomUrl) roomUrl.substringAfterLast('/') else roomUrl
        }

    // ── Camera state ─────────────────────────────────────────────────────────
    val hasCameraPermission =
        ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
    var cameraEnabled by remember { mutableStateOf(hasCameraPermission) }
    var isFrontCamera by remember { mutableStateOf(true) }
    val cameraPreviewRef = remember { mutableStateOf<LocalCameraPreview?>(null) }

    // ── Audio state ───────────────────────────────────────────────────────────
    val hasMicPermission =
        ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
    var micEnabled by remember { mutableStateOf(hasMicPermission) }
    // "computer_audio" | "no_audio"
    var audioMode by remember { mutableStateOf("computer_audio") }
    var vuLevel by remember { mutableFloatStateOf(0f) }

    // ── Permission launchers ──────────────────────────────────────────────────
    val cameraPermissionLauncher =
        rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted -> if (granted) cameraEnabled = true }

    val micPermissionLauncher =
        rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted -> if (granted) micEnabled = true }

    // ── Audio device state ────────────────────────────────────────────────────
    val audioManager = remember { context.getSystemService(Context.AUDIO_SERVICE) as AudioManager }
    var audioDeviceRefreshKey by remember { mutableStateOf(0) }

    DisposableEffect(Unit) {
        val listener =
            object : android.media.AudioDeviceCallback() {
                override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) {
                    audioDeviceRefreshKey++
                }

                override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) {
                    audioDeviceRefreshKey++
                }
            }
        audioManager.registerAudioDeviceCallback(listener, null)
        onDispose { audioManager.unregisterAudioDeviceCallback(listener) }
    }

    val audioOutputDevices =
        remember(audioDeviceRefreshKey) {
            audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
                .filter {
                    it.type in
                        listOf(
                            AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
                            AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
                            AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER,
                            AudioDeviceInfo.TYPE_HEARING_AID,
                            AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
                            AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_USB_DEVICE,
                        )
                }
                .distinctBy { it.type }
        }

    val audioInputDevices =
        remember(audioDeviceRefreshKey) {
            audioManager.getDevices(AudioManager.GET_DEVICES_INPUTS)
                .filter {
                    it.type in
                        listOf(
                            AudioDeviceInfo.TYPE_BUILTIN_MIC,
                            AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_BLE_HEADSET,
                            AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_USB_HEADSET,
                        )
                }
                .distinctBy { it.type }
        }

    var selectedOutputRoute by remember { mutableStateOf<String?>(null) }
    var selectedInputRoute by remember { mutableStateOf<String?>(null) }
    var selectedOutputDeviceRef by remember { mutableStateOf<AudioDeviceInfo?>(null) }
    var selectedInputDeviceRef by remember { mutableStateOf<AudioDeviceInfo?>(null) }
    var showOutputRouteMenu by remember { mutableStateOf(false) }
    var showInputRouteMenu by remember { mutableStateOf(false) }

    // Auto-select best device, fallback to speaker on BT disconnect
    LaunchedEffect(audioDeviceRefreshKey) {
        val externalOutput =
            audioOutputDevices.firstOrNull {
                it.type in
                    listOf(
                        AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
                        AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER,
                        AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
                        AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_USB_DEVICE,
                        AudioDeviceInfo.TYPE_HEARING_AID,
                    )
            }
        selectedOutputRoute =
            audioDeviceTypeLabel(
                (externalOutput ?: audioOutputDevices.firstOrNull())?.type ?: AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
                lang,
            )
        val externalInput =
            audioInputDevices.firstOrNull {
                it.type in
                    listOf(
                        AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_BLE_HEADSET,
                        AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_USB_HEADSET,
                    )
            }
        selectedInputRoute =
            audioDeviceTypeLabel(
                (externalInput ?: audioInputDevices.firstOrNull())?.type ?: AudioDeviceInfo.TYPE_BUILTIN_MIC,
                lang,
            )
    }

    // ── Background filter sheet ───────────────────────────────────────────────
    var showFilterSheet by remember { mutableStateOf(false) }
    var backgroundMode by remember {
        mutableStateOf(
            try {
                VisioManager.client.getBackgroundMode()
            } catch (_: Exception) {
                "off"
            },
        )
    }

    // ── Waiting state ─────────────────────────────────────────────────────────
    var waitingState by remember { mutableStateOf<WaitingState>(WaitingState.Idle) }
    val connectionState by VisioManager.connectionState.collectAsState()

    // ── VU meter via AudioRecord ──────────────────────────────────────────────
    LaunchedEffect(micEnabled, audioMode) {
        val currentHasMic =
            ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) ==
                PackageManager.PERMISSION_GRANTED
        if (!micEnabled || audioMode != "computer_audio" || !currentHasMic) {
            vuLevel = 0f
            return@LaunchedEffect
        }
        val sampleRate = 16_000
        val bufferSize =
            AudioRecord.getMinBufferSize(
                sampleRate,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
            ).coerceAtLeast(1024)

        @SuppressLint("MissingPermission")
        val recorder =
            AudioRecord(
                MediaRecorder.AudioSource.MIC,
                sampleRate,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                bufferSize,
            )
        recorder.startRecording()
        try {
            val buf = ShortArray(bufferSize / 2)
            while (isActive) {
                val read = recorder.read(buf, 0, buf.size)
                if (read > 0) {
                    var sum = 0.0
                    for (i in 0 until read) sum += buf[i].toLong() * buf[i].toLong()
                    val rms = sqrt(sum / read)
                    val normalized = ((rms / 32768.0) * 25.0).toFloat().coerceIn(0f, 1f)
                    vuLevel = normalized
                }
                delay(50)
            }
        } finally {
            recorder.stop()
            recorder.release()
        }
    }

    // ── Observe lobby-denied event ────────────────────────────────────────────
    val lobbyDenied by VisioManager.lobbyDenied.collectAsState()
    LaunchedEffect(lobbyDenied) {
        if (lobbyDenied && waitingState is WaitingState.Waiting) {
            waitingState = WaitingState.Denied
            VisioManager.lobbyDenied.value = false
        }
    }

    // ── React to Connected: navigate into call ────────────────────────────────
    LaunchedEffect(connectionState) {
        if (connectionState is ConnectionState.Connected && waitingState is WaitingState.Waiting) {
            waitingState = WaitingState.Idle
            onJoin(displayName)
        }
    }

    // ── Layout ────────────────────────────────────────────────────────────────
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .statusBarsPadding()
                .navigationBarsPadding()
                .imePadding(),
    ) {
        // Top bar
        TopAppBar(
            title = {
                Text(
                    text = roomSlug,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    fontWeight = FontWeight.Medium,
                )
            },
            navigationIcon = {
                IconButton(onClick = onCancel) {
                    Icon(
                        painter = painterResource(R.drawable.ri_arrow_left_s_line),
                        contentDescription = "Back",
                        tint = MaterialTheme.colorScheme.onSurface,
                    )
                }
            },
            colors =
                TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                ),
        )

        Column(
            modifier =
                Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            // ── Display name field ────────────────────────────────────────────
            PreJoinSectionLabel(
                text = Strings.t("home.displayName", lang),
                isDark = isDark,
            )
            TextField(
                value = displayName,
                onValueChange = { displayName = it },
                placeholder = {
                    Text(
                        Strings.t("home.displayName.placeholder", lang),
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    )
                },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
                colors =
                    TextFieldDefaults.colors(
                        focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                        unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                        cursorColor = VisioColors.Primary500,
                        focusedTextColor = MaterialTheme.colorScheme.onSurface,
                        unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                    ),
                shape = RoundedCornerShape(12.dp),
            )

            // ── Camera preview ────────────────────────────────────────────────
            PreJoinCameraSection(
                cameraEnabled = cameraEnabled,
                hasCameraPermission = hasCameraPermission,
                isFrontCamera = isFrontCamera,
                isDark = isDark,
                lang = lang,
                backgroundMode = backgroundMode,
                cameraPreviewRef = cameraPreviewRef,
                onToggleCamera = { newEnabled ->
                    if (newEnabled) {
                        val hasPerm =
                            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                                PackageManager.PERMISSION_GRANTED
                        if (hasPerm) {
                            cameraEnabled = true
                        } else {
                            cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
                        }
                    } else {
                        cameraEnabled = false
                    }
                },
                onFlipCamera = { isFrontCamera = !isFrontCamera },
            )

            // ── Audio config ──────────────────────────────────────────────────
            PreJoinAudioSection(
                audioMode = audioMode,
                onAudioModeChange = { audioMode = it },
                micEnabled = micEnabled,
                onMicToggle = { newValue ->
                    if (newValue) {
                        val hasPerm =
                            ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) ==
                                PackageManager.PERMISSION_GRANTED
                        if (hasPerm) {
                            micEnabled = true
                        } else {
                            micPermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
                        }
                    } else {
                        micEnabled = false
                    }
                },
                vuLevel = vuLevel,
                audioInputDevices = audioInputDevices,
                audioOutputDevices = audioOutputDevices,
                selectedInputRoute = selectedInputRoute,
                selectedOutputRoute = selectedOutputRoute,
                showInputRouteMenu = showInputRouteMenu,
                showOutputRouteMenu = showOutputRouteMenu,
                onShowInputRouteMenu = { showInputRouteMenu = it },
                onShowOutputRouteMenu = { showOutputRouteMenu = it },
                onSelectInputRoute = { label ->
                    selectedInputRoute = label
                    selectedInputDeviceRef = audioInputDevices.firstOrNull { audioDeviceLabel(it, lang) == label }
                },
                onSelectOutputRoute = { label ->
                    selectedOutputRoute = label
                    selectedOutputDeviceRef = audioOutputDevices.firstOrNull { audioDeviceLabel(it, lang) == label }
                },
                isDark = isDark,
                lang = lang,
            )

            // ── Background filter button ───────────────────────────────────────
            OutlinedButton(
                onClick = { showFilterSheet = true },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp),
            ) {
                val bgLabel =
                    when {
                        backgroundMode == "off" -> Strings.t("settings.incall.bgOff", lang)
                        backgroundMode == "blur" -> Strings.t("settings.incall.bgBlur", lang)
                        backgroundMode.startsWith("image:") -> {
                            val id = backgroundMode.removePrefix("image:")
                            "Background $id"
                        }
                        else -> Strings.t(KEY_SETTINGS_BACKGROUND, lang)
                    }
                Text(
                    text = "${Strings.t(KEY_SETTINGS_BACKGROUND, lang)}: $bgLabel",
                    modifier = Modifier.padding(vertical = 4.dp),
                )
            }

            // ── Waiting state banner ──────────────────────────────────────────
            PreJoinWaitingBanner(
                waitingState = waitingState,
                isDark = isDark,
                lang = lang,
            )
        }

        // ── Action buttons ────────────────────────────────────────────────────
        Column(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val isJoining = waitingState is WaitingState.Waiting

            Button(
                onClick = {
                    if (isJoining) return@Button
                    coroutineScope.launch(Dispatchers.IO) {
                        try {
                            // Save settings
                            val name = displayName.trim().ifBlank { null }
                            if (name != null) {
                                VisioManager.client.setDisplayName(name)
                                withContext(Dispatchers.Main) {
                                    VisioManager.updateDisplayName(name)
                                }
                            }
                            VisioManager.client.setMicEnabledOnJoin(
                                micEnabled && audioMode == "computer_audio",
                            )
                            VisioManager.client.setCameraEnabledOnJoin(cameraEnabled)

                            withContext(Dispatchers.Main) {
                                waitingState = WaitingState.Waiting
                            }

                            // Connect
                            VisioManager.client.connect(roomUrl, displayName.trim())
                            VisioManager.startAudioPlayout()

                            // Apply output device from lobby
                            selectedOutputDeviceRef?.let { device ->
                                try {
                                    VisioManager.setAudioOutputDevice(device)
                                } catch (_: Exception) {
                                }
                            }

                            // Enable mic/camera based on lobby selections
                            if (micEnabled && audioMode == "computer_audio") {
                                try {
                                    VisioManager.client.setMicrophoneEnabled(true)
                                    VisioManager.startAudioCapture(selectedInputDeviceRef)
                                } catch (_: Exception) {
                                }
                            }
                            if (cameraEnabled) {
                                try {
                                    VisioManager.client.setCameraEnabled(true)
                                    VisioManager.startCameraCapture()
                                } catch (_: Exception) {
                                }
                            }
                        } catch (e: Exception) {
                            withContext(Dispatchers.Main) {
                                waitingState = WaitingState.Idle
                            }
                        }
                    }

                    // 60-second timeout
                    coroutineScope.launch {
                        delay(60_000)
                        if (waitingState is WaitingState.Waiting) {
                            waitingState = WaitingState.Timeout
                            withContext(Dispatchers.IO) {
                                try {
                                    VisioManager.cancelLobby()
                                    VisioManager.disconnect()
                                } catch (_: Exception) {
                                }
                            }
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isJoining,
                colors =
                    ButtonDefaults.buttonColors(
                        containerColor = VisioColors.Primary500,
                        contentColor = VisioColors.White,
                        disabledContainerColor = VisioColors.PrimaryDark300,
                        disabledContentColor = VisioColors.Greyscale400,
                    ),
                shape = RoundedCornerShape(12.dp),
            ) {
                if (isJoining) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(18.dp),
                        strokeWidth = 2.dp,
                        color = VisioColors.White,
                    )
                } else {
                    Text(
                        Strings.t("home.join", lang),
                        fontSize = 16.sp,
                        modifier = Modifier.padding(vertical = 4.dp),
                    )
                }
            }

            if (isJoining) {
                OutlinedButton(
                    onClick = {
                        waitingState = WaitingState.Idle
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                VisioManager.cancelLobby()
                                VisioManager.disconnect()
                            } catch (_: Exception) {
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp),
                ) {
                    Text(Strings.t("lobby.cancel", lang))
                }
            }
        }
    }

    // ── Background filter bottom sheet ────────────────────────────────────────
    if (showFilterSheet) {
        BackgroundFilterSheet(
            currentMode = backgroundMode,
            isDark = isDark,
            lang = lang,
            onSelect = { mode ->
                backgroundMode = mode
                showFilterSheet = false
            },
            onDismiss = { showFilterSheet = false },
        )
    }
}

// ── Background filter bottom sheet ────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun BackgroundFilterSheet(
    currentMode: String,
    isDark: Boolean,
    lang: String,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val context = LocalContext.current
    val coroutineScope = rememberCoroutineScope()
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = if (isDark) VisioColors.PrimaryDark75 else MaterialTheme.colorScheme.surface,
    ) {
        Column(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp)
                    .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                text = Strings.t(KEY_SETTINGS_BACKGROUND, lang),
                style = MaterialTheme.typography.titleMedium,
                color = if (isDark) VisioColors.White else MaterialTheme.colorScheme.onSurface,
                fontWeight = FontWeight.SemiBold,
            )

            // None / Blur row
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FilterChip(
                    label = Strings.t("settings.incall.bgOff", lang),
                    selected = currentMode == "off",
                    isDark = isDark,
                    onClick = {
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                VisioManager.client.setBackgroundMode("off")
                            } catch (_: Exception) {
                            }
                        }
                        onSelect("off")
                    },
                    modifier = Modifier.weight(1f),
                )
                FilterChip(
                    label = Strings.t("settings.incall.bgBlur", lang),
                    selected = currentMode == "blur",
                    isDark = isDark,
                    onClick = {
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                VisioManager.client.setBackgroundMode("blur")
                            } catch (_: Exception) {
                            }
                        }
                        onSelect("blur")
                    },
                    modifier = Modifier.weight(1f),
                )
            }

            // Image grid
            val imageIds = (1..8).toList()
            LazyVerticalGrid(
                columns = GridCells.Fixed(4),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.height(180.dp),
            ) {
                items(imageIds) { id ->
                    val isSelected = currentMode == "image:$id"
                    val bitmap =
                        remember(id) {
                            try {
                                BitmapFactory.decodeStream(
                                    context.assets.open("backgrounds/thumbnails/$id.jpg"),
                                )
                            } catch (_: Exception) {
                                null
                            }
                        }
                    Box(
                        modifier =
                            Modifier
                                .aspectRatio(1f)
                                .clip(RoundedCornerShape(8.dp))
                                .then(
                                    if (isSelected) {
                                        Modifier.border(2.dp, VisioColors.Primary500, RoundedCornerShape(8.dp))
                                    } else {
                                        Modifier
                                    },
                                )
                                .background(
                                    if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                                )
                                .clickable {
                                    coroutineScope.launch(Dispatchers.IO) {
                                        try {
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
                                        } catch (_: Exception) {
                                        }
                                    }
                                    onSelect("image:$id")
                                },
                    ) {
                        if (bitmap != null) {
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
        }
    }
}

// ── Extracted sub-composables ─────────────────────────────────────────────────

@Composable
private fun PreJoinCameraSection(
    cameraEnabled: Boolean,
    hasCameraPermission: Boolean,
    isFrontCamera: Boolean,
    isDark: Boolean,
    lang: String,
    backgroundMode: String,
    cameraPreviewRef: androidx.compose.runtime.MutableState<LocalCameraPreview?>,
    onToggleCamera: (Boolean) -> Unit,
    onFlipCamera: () -> Unit,
) {
    PreJoinSectionLabel(
        text = Strings.t("settings.incall.camera", lang),
        isDark = isDark,
    )

    Box(
        modifier =
            Modifier
                .fillMaxWidth()
                .aspectRatio(4f / 3f)
                .clip(RoundedCornerShape(12.dp))
                .background(if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant),
        contentAlignment = Alignment.Center,
    ) {
        if (cameraEnabled && hasCameraPermission) {
            AndroidView(
                factory = { ctx ->
                    LocalCameraPreview(ctx, isFrontCamera).also { preview ->
                        cameraPreviewRef.value = preview
                    }
                },
                update = { preview ->
                    preview.switchCamera(isFrontCamera)
                },
                modifier =
                    Modifier
                        .fillMaxSize()
                        .clip(RoundedCornerShape(12.dp)),
            )
        } else {
            Icon(
                imageVector = Icons.Filled.VideocamOff,
                contentDescription = null,
                tint = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                modifier = Modifier.size(48.dp),
            )
        }

        // Camera controls overlay
        Row(
            modifier =
                Modifier
                    .align(Alignment.BottomEnd)
                    .padding(8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (cameraEnabled && hasCameraPermission) {
                Box(
                    modifier =
                        Modifier
                            .size(36.dp)
                            .background(Color.Black.copy(alpha = 0.4f), CircleShape)
                            .clickable { onFlipCamera() },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = Icons.Filled.Cameraswitch,
                        contentDescription = Strings.t("call.switchCamera", lang),
                        tint = VisioColors.White,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
            Box(
                modifier =
                    Modifier
                        .size(36.dp)
                        .background(
                            if (cameraEnabled) VisioColors.Primary500 else Color.Black.copy(alpha = 0.4f),
                            CircleShape,
                        )
                        .clickable { onToggleCamera(!cameraEnabled) },
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = if (cameraEnabled) Icons.Filled.Videocam else Icons.Filled.VideocamOff,
                    contentDescription =
                        if (cameraEnabled) {
                            Strings.t("control.camOff", lang)
                        } else {
                            Strings.t("control.camOn", lang)
                        },
                    tint = VisioColors.White,
                    modifier = Modifier.size(20.dp),
                )
            }
        }
    }
}

@Composable
private fun PreJoinAudioSection(
    audioMode: String,
    onAudioModeChange: (String) -> Unit,
    micEnabled: Boolean,
    onMicToggle: (Boolean) -> Unit,
    vuLevel: Float,
    audioInputDevices: List<AudioDeviceInfo>,
    audioOutputDevices: List<AudioDeviceInfo>,
    selectedInputRoute: String?,
    selectedOutputRoute: String?,
    showInputRouteMenu: Boolean,
    showOutputRouteMenu: Boolean,
    onShowInputRouteMenu: (Boolean) -> Unit,
    onShowOutputRouteMenu: (Boolean) -> Unit,
    onSelectInputRoute: (String) -> Unit,
    onSelectOutputRoute: (String) -> Unit,
    isDark: Boolean,
    lang: String,
) {
    PreJoinSectionLabel(
        text = Strings.t("settings.incall.micro", lang),
        isDark = isDark,
    )

    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(
                    if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    RoundedCornerShape(12.dp),
                )
                .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        AudioModeRadio(
            label = Strings.t("prejoin.computerAudio", lang),
            selected = audioMode == "computer_audio",
            onClick = { onAudioModeChange("computer_audio") },
            isDark = isDark,
        )

        if (audioMode == "computer_audio") {
            // Mic toggle row
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(start = 40.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = Strings.t(KEY_DEVICE_MICROPHONE, lang),
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                )
                Switch(
                    checked = micEnabled,
                    onCheckedChange = { onMicToggle(it) },
                    colors =
                        SwitchDefaults.colors(
                            checkedThumbColor = VisioColors.White,
                            checkedTrackColor = VisioColors.Primary500,
                            uncheckedThumbColor = VisioColors.Greyscale400,
                            uncheckedTrackColor =
                                if (isDark) VisioColors.PrimaryDark300 else VisioColors.LightBorder,
                        ),
                )
            }

            // VU meter
            if (micEnabled) {
                Column(modifier = Modifier.padding(start = 40.dp)) {
                    Text(
                        text = "Mic level",
                        style = MaterialTheme.typography.labelSmall,
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    LinearProgressIndicator(
                        progress = { vuLevel },
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .clip(RoundedCornerShape(3.dp)),
                        color = Color(0xFF22C55E),
                        trackColor = if (isDark) VisioColors.PrimaryDark300 else VisioColors.LightBorder,
                    )
                }
            }

            PreJoinAudioRouteSelectors(
                audioInputDevices = audioInputDevices,
                audioOutputDevices = audioOutputDevices,
                selectedInputRoute = selectedInputRoute,
                selectedOutputRoute = selectedOutputRoute,
                showInputRouteMenu = showInputRouteMenu,
                showOutputRouteMenu = showOutputRouteMenu,
                onShowInputRouteMenu = onShowInputRouteMenu,
                onShowOutputRouteMenu = onShowOutputRouteMenu,
                onSelectInputRoute = onSelectInputRoute,
                onSelectOutputRoute = onSelectOutputRoute,
                lang = lang,
            )
        }

        AudioModeRadio(
            label = Strings.t("prejoin.noAudio", lang),
            selected = audioMode == "no_audio",
            onClick = { onAudioModeChange("no_audio") },
            isDark = isDark,
        )
    }
}

@Composable
private fun PreJoinAudioRouteSelectors(
    audioInputDevices: List<AudioDeviceInfo>,
    audioOutputDevices: List<AudioDeviceInfo>,
    selectedInputRoute: String?,
    selectedOutputRoute: String?,
    showInputRouteMenu: Boolean,
    showOutputRouteMenu: Boolean,
    onShowInputRouteMenu: (Boolean) -> Unit,
    onShowOutputRouteMenu: (Boolean) -> Unit,
    onSelectInputRoute: (String) -> Unit,
    onSelectOutputRoute: (String) -> Unit,
    lang: String,
) {
    // Mic input selector
    if (audioInputDevices.size > 1) {
        Box(modifier = Modifier.padding(start = 40.dp)) {
            OutlinedButton(
                onClick = { onShowInputRouteMenu(true) },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
            ) {
                Icon(
                    imageVector = Icons.Filled.Mic,
                    contentDescription = null,
                    tint = VisioColors.Primary500,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(modifier = Modifier.size(8.dp))
                Text(
                    text = selectedInputRoute ?: Strings.t(KEY_DEVICE_MICROPHONE, lang),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.weight(1f),
                )
            }
            DropdownMenu(
                expanded = showInputRouteMenu,
                onDismissRequest = { onShowInputRouteMenu(false) },
            ) {
                audioInputDevices.forEach { device ->
                    val label = audioDeviceTypeLabel(device.type, lang)
                    DropdownMenuItem(
                        text = { Text(label) },
                        onClick = {
                            onSelectInputRoute(label)
                            onShowInputRouteMenu(false)
                        },
                    )
                }
            }
        }
    }

    // Output route selector
    Box(modifier = Modifier.padding(start = 40.dp)) {
        OutlinedButton(
            onClick = { onShowOutputRouteMenu(true) },
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(8.dp),
        ) {
            Icon(
                imageVector = Icons.AutoMirrored.Filled.VolumeUp,
                contentDescription = null,
                tint = VisioColors.Primary500,
                modifier = Modifier.size(18.dp),
            )
            Spacer(modifier = Modifier.size(8.dp))
            Text(
                text = selectedOutputRoute ?: Strings.t("prejoin.audioRoute", lang),
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.weight(1f),
            )
        }
        DropdownMenu(
            expanded = showOutputRouteMenu,
            onDismissRequest = { onShowOutputRouteMenu(false) },
        ) {
            audioOutputDevices.forEach { device ->
                val label = audioDeviceTypeLabel(device.type, lang)
                DropdownMenuItem(
                    text = { Text(label) },
                    onClick = {
                        onSelectOutputRoute(label)
                        onShowOutputRouteMenu(false)
                    },
                )
            }
        }
    }
}

@Composable
private fun PreJoinWaitingBanner(
    waitingState: WaitingState,
    isDark: Boolean,
    lang: String,
) {
    when (waitingState) {
        is WaitingState.Waiting -> {
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .background(
                            VisioColors.Primary500.copy(alpha = 0.15f),
                            RoundedCornerShape(12.dp),
                        )
                        .padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    strokeWidth = 2.dp,
                    color = VisioColors.Primary500,
                )
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = Strings.t("lobby.waiting", lang),
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Medium,
                        color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    )
                    Text(
                        text = Strings.t("lobby.waitingDesc", lang),
                        style = MaterialTheme.typography.bodySmall,
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    )
                }
            }
        }
        is WaitingState.Denied -> {
            Text(
                text = Strings.t("lobby.denied", lang),
                style = MaterialTheme.typography.bodyMedium,
                color = VisioColors.Error500,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .background(
                            if (isDark) VisioColors.Error200 else VisioColors.LightErrorBg,
                            RoundedCornerShape(12.dp),
                        )
                        .padding(16.dp),
            )
        }
        is WaitingState.Timeout -> {
            Text(
                text = Strings.t("lobby.timeout", lang),
                style = MaterialTheme.typography.bodyMedium,
                color = VisioColors.Error500,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .background(
                            if (isDark) VisioColors.Error200 else VisioColors.LightErrorBg,
                            RoundedCornerShape(12.dp),
                        )
                        .padding(16.dp),
            )
        }
        else -> {}
    }
}

// ── Reusable chip for filter selection ────────────────────────────────────────

@Composable
private fun FilterChip(
    label: String,
    selected: Boolean,
    isDark: Boolean,
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
                    if (selected) {
                        VisioColors.Primary500
                    } else if (isDark) {
                        VisioColors.PrimaryDark100
                    } else {
                        VisioColors.LightSurfaceVariant
                    },
                )
                .clickable(onClick = onClick)
                .padding(horizontal = 12.dp),
    ) {
        Text(
            text = label,
            color = if (selected || isDark) VisioColors.White else VisioColors.LightOnBackground,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

// ── Section label ─────────────────────────────────────────────────────────────

@Composable
private fun AudioModeRadio(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    isDark: Boolean,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .selectable(
                    selected = selected,
                    onClick = onClick,
                    role = Role.RadioButton,
                ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(
            selected = selected,
            onClick = onClick,
            colors =
                RadioButtonDefaults.colors(
                    selectedColor = VisioColors.Primary500,
                    unselectedColor = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                ),
        )
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
        )
    }
}

@Composable
private fun PreJoinSectionLabel(
    text: String,
    isDark: Boolean,
) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleSmall,
        fontWeight = FontWeight.SemiBold,
        color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
    )
}

private val BUILTIN_TYPES =
    listOf(
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
        AudioDeviceInfo.TYPE_BUILTIN_MIC,
    )

private fun audioDeviceLabel(
    device: AudioDeviceInfo,
    lang: String,
): String {
    if (device.type !in BUILTIN_TYPES) {
        val name = device.productName?.toString()?.ifBlank { null }
        if (name != null) return name
    }
    return audioDeviceTypeLabel(device.type, lang)
}

private fun audioDeviceTypeLabel(
    type: Int,
    lang: String,
): String =
    when (type) {
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> Strings.t("audio.speaker", lang)
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> Strings.t("audio.earpiece", lang)
        AudioDeviceInfo.TYPE_BUILTIN_MIC -> Strings.t(KEY_DEVICE_MICROPHONE, lang)
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> Strings.t("audio.bluetooth", lang)
        AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER -> Strings.t("audio.bluetooth", lang)
        AudioDeviceInfo.TYPE_HEARING_AID -> Strings.t("audio.hearingAid", lang)
        AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADPHONES -> Strings.t("audio.wiredHeadset", lang)
        AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_USB_DEVICE -> Strings.t("audio.usbHeadset", lang)
        else -> "Audio"
    }
