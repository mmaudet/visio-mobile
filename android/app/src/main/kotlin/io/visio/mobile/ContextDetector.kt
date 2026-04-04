package io.visio.mobile

import android.bluetooth.BluetoothClass
import android.bluetooth.BluetoothHeadset
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import uniffi.visio.NetworkType

class ContextDetector(private val context: Context) {
    private val connectivityManager =
        context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private val sensorManager =
        context.getSystemService(Context.SENSOR_SERVICE) as SensorManager
    private val audioManager =
        context.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    private val mainHandler = Handler(Looper.getMainLooper())

    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var accelerometerListener: SensorEventListener? = null
    private var audioDeviceCallback: AudioDeviceCallback? = null
    private var bluetoothReceiver: android.content.BroadcastReceiver? = null
    private var scoReceiver: android.content.BroadcastReceiver? = null

    // SCO confirmation: pending BT device awaiting SCO profile connection
    private var pendingBtConnect: Boolean = false
    private val scoTimeoutMs = 3000L
    private val scoTimeoutRunnable = Runnable {
        Log.w(TAG, "SCO confirmation timeout — ignoring pending BT connect")
        pendingBtConnect = false
    }

    // Rapid connect/disconnect debounce
    private val btDebounceMs = 500L
    private var pendingConnectRunnable: Runnable? = null
    private var pendingDisconnectRunnable: Runnable? = null

    private var lastReportedMotion = false
    private var lastSignificantMotionMs = 0L // last time we saw a significant accel event
    private val motionThreshold = 2.5f // m/s² deviation from gravity to count as motion
    private val motionCooldownMs = 15000L // stay in pedestrian 15s after last motion event

    companion object {
        private const val TAG = "ContextDetector"
    }

    fun start() {
        Log.i(TAG, "Starting context detection (network, motion, bluetooth)")
        startNetworkMonitoring()
        startMotionDetection()
        startBluetoothMonitoring()
        reportCurrentNetworkType()
        reportBluetoothCarKit()
    }

    fun stop() {
        networkCallback?.let { connectivityManager.unregisterNetworkCallback(it) }
        accelerometerListener?.let { sensorManager.unregisterListener(it) }
        audioDeviceCallback?.let { audioManager.unregisterAudioDeviceCallback(it) }
        bluetoothReceiver?.let { context.unregisterReceiver(it) }
        scoReceiver?.let { context.unregisterReceiver(it) }
        // Cancel pending debounce/timeout callbacks
        mainHandler.removeCallbacks(scoTimeoutRunnable)
        pendingConnectRunnable?.let { mainHandler.removeCallbacks(it) }
        pendingDisconnectRunnable?.let { mainHandler.removeCallbacks(it) }
        pendingBtConnect = false
        networkCallback = null
        accelerometerListener = null
        audioDeviceCallback = null
        bluetoothReceiver = null
        scoReceiver = null
    }

    private fun startNetworkMonitoring() {
        val request = NetworkRequest.Builder().build()
        val callback =
            object : ConnectivityManager.NetworkCallback() {
                override fun onCapabilitiesChanged(
                    network: Network,
                    caps: NetworkCapabilities,
                ) {
                    val type =
                        when {
                            caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> NetworkType.WIFI
                            caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> NetworkType.CELLULAR
                            else -> NetworkType.UNKNOWN
                        }
                    Log.d(TAG, "Network type: $type")
                    try {
                        VisioManager.client.reportNetworkType(type)
                    } catch (_: Exception) {
                        // No-op
                    }
                }

                override fun onLost(network: Network) {
                    Log.d(TAG, "Network lost")
                    try {
                        VisioManager.client.reportNetworkType(NetworkType.UNKNOWN)
                    } catch (_: Exception) {
                        // No-op
                    }
                }
            }
        connectivityManager.registerNetworkCallback(request, callback)
        networkCallback = callback
    }

    private fun reportCurrentNetworkType() {
        val caps = connectivityManager.getNetworkCapabilities(connectivityManager.activeNetwork)
        val type =
            when {
                caps == null -> NetworkType.UNKNOWN
                caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> NetworkType.WIFI
                caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> NetworkType.CELLULAR
                else -> NetworkType.UNKNOWN
            }
        try {
            VisioManager.client.reportNetworkType(type)
        } catch (_: Exception) {
            // No-op
        }
    }

    @Suppress("kotlin:S3776")
    private fun startMotionDetection() {
        val accelerometer = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)
        if (accelerometer == null) {
            Log.w(TAG, "No accelerometer sensor available")
            return
        }
        Log.d(TAG, "Accelerometer found: ${accelerometer.name}")
        val listener =
            object : SensorEventListener {
                override fun onSensorChanged(event: SensorEvent) {
                    val x = event.values[0]
                    val y = event.values[1]
                    val z = event.values[2]
                    val magnitude = kotlin.math.sqrt((x * x + y * y + z * z).toDouble()).toFloat()
                    val deviation = kotlin.math.abs(magnitude - SensorManager.GRAVITY_EARTH)
                    val now = System.currentTimeMillis()

                    // Any significant acceleration extends the "moving" window
                    if (deviation > motionThreshold) {
                        val wasMoving =
                            lastSignificantMotionMs > 0L &&
                                now - lastSignificantMotionMs < motionCooldownMs
                        lastSignificantMotionMs = now
                        if (!wasMoving && !lastReportedMotion) {
                            // First motion event — switch to moving
                            lastReportedMotion = true
                            Log.d(TAG, "Motion detected (dev=${"%.2f".format(deviation)})")
                            try {
                                VisioManager.client.reportMotionDetected(true)
                            } catch (_: Exception) {
                                // No-op
                            }
                        }
                    } else if (
                        lastReportedMotion &&
                        lastSignificantMotionMs > 0L &&
                        now - lastSignificantMotionMs > motionCooldownMs
                    ) {
                        // Cooldown expired — no significant motion for 15s
                        lastReportedMotion = false
                        Log.d(TAG, "Motion stopped (${motionCooldownMs}ms cooldown expired)")
                        try {
                            VisioManager.client.reportMotionDetected(false)
                        } catch (_: Exception) {
                            // No-op
                        }
                    }
                }

                override fun onAccuracyChanged(
                    sensor: Sensor?,
                    accuracy: Int,
                ) {
                    // No-op
                }
            }
        sensorManager.registerListener(listener, accelerometer, SensorManager.SENSOR_DELAY_NORMAL)
        accelerometerListener = listener
    }

    @Suppress("kotlin:S3776")
    private fun startBluetoothMonitoring() {
        val callback =
            object : AudioDeviceCallback() {
                override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) {
                    Log.d(TAG, "Audio devices added: ${addedDevices.map { "${it.productName}(${it.type})" }}")
                    reportBluetoothCarKit()
                    // Check if a Bluetooth audio device was added
                    val btDevice =
                        addedDevices.firstOrNull { device ->
                            device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                                device.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                                device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
                        }
                    if (btDevice != null) {
                        Log.i(TAG, "Bluetooth audio device detected: ${btDevice.productName}, waiting for SCO confirmation")
                        // Don't route immediately — wait for SCO profile confirmation
                        pendingBtConnect = true
                        mainHandler.removeCallbacks(scoTimeoutRunnable)
                        mainHandler.postDelayed(scoTimeoutRunnable, scoTimeoutMs)
                    }
                }

                override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) {
                    Log.d(TAG, "Audio devices removed: ${removedDevices.map { "${it.productName}(${it.type})" }}")
                    reportBluetoothCarKit()
                    // Check if removed device was Bluetooth audio
                    val wasBt =
                        removedDevices.any { device ->
                            device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                                device.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                                device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
                        }
                    if (wasBt) {
                        // Cancel any pending connect — device disconnected before SCO confirmed
                        pendingBtConnect = false
                        mainHandler.removeCallbacks(scoTimeoutRunnable)
                        // Debounce disconnect to avoid thrashing
                        debouncedDisconnect()
                    }
                }
            }
        audioManager.registerAudioDeviceCallback(callback, mainHandler)
        audioDeviceCallback = callback

        // BroadcastReceiver for reliable Bluetooth disconnect detection
        val receiver =
            object : android.content.BroadcastReceiver() {
                override fun onReceive(
                    ctx: android.content.Context?,
                    intent: android.content.Intent?,
                ) {
                    when (intent?.action) {
                        android.bluetooth.BluetoothDevice.ACTION_ACL_DISCONNECTED,
                        android.bluetooth.BluetoothDevice.ACTION_ACL_CONNECTED,
                        -> {
                            val device =
                                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                                    intent.getParcelableExtra(
                                        android.bluetooth.BluetoothDevice.EXTRA_DEVICE,
                                        android.bluetooth.BluetoothDevice::class.java,
                                    )
                                } else {
                                    @Suppress("DEPRECATION")
                                    intent.getParcelableExtra(android.bluetooth.BluetoothDevice.EXTRA_DEVICE)
                                }
                            Log.d(TAG, "Bluetooth ACL ${intent.action}: ${device?.name}")
                            mainHandler.postDelayed({
                                reportBluetoothCarKit()
                            }, 500)
                        }
                    }
                }
            }
        val filter =
            android.content.IntentFilter().apply {
                addAction(android.bluetooth.BluetoothDevice.ACTION_ACL_DISCONNECTED)
                addAction(android.bluetooth.BluetoothDevice.ACTION_ACL_CONNECTED)
            }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            context.registerReceiver(receiver, filter)
        }
        bluetoothReceiver = receiver

        // SCO profile confirmation receiver — routes audio only after SCO is established
        val scoRcv =
            object : android.content.BroadcastReceiver() {
                override fun onReceive(
                    ctx: android.content.Context?,
                    intent: android.content.Intent?,
                ) {
                    if (intent?.action != BluetoothHeadset.ACTION_CONNECTION_STATE_CHANGED) return
                    val state = intent.getIntExtra(BluetoothProfile.EXTRA_STATE, -1)
                    val device =
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                            intent.getParcelableExtra(
                                android.bluetooth.BluetoothDevice.EXTRA_DEVICE,
                                android.bluetooth.BluetoothDevice::class.java,
                            )
                        } else {
                            @Suppress("DEPRECATION")
                            intent.getParcelableExtra(android.bluetooth.BluetoothDevice.EXTRA_DEVICE)
                        }
                    Log.d(TAG, "SCO connection state: $state for ${device?.name}")
                    when (state) {
                        BluetoothProfile.STATE_CONNECTED -> {
                            if (pendingBtConnect) {
                                pendingBtConnect = false
                                mainHandler.removeCallbacks(scoTimeoutRunnable)
                                Log.i(TAG, "SCO confirmed for ${device?.name}, routing audio via debounce")
                                debouncedConnect()
                            }
                        }
                        BluetoothProfile.STATE_DISCONNECTED -> {
                            if (pendingBtConnect) {
                                pendingBtConnect = false
                                mainHandler.removeCallbacks(scoTimeoutRunnable)
                                Log.w(TAG, "SCO disconnected before confirmation — ignoring pending BT connect")
                            }
                            debouncedDisconnect()
                        }
                    }
                }
            }
        val scoFilter = android.content.IntentFilter(BluetoothHeadset.ACTION_CONNECTION_STATE_CHANGED)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            context.registerReceiver(scoRcv, scoFilter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            context.registerReceiver(scoRcv, scoFilter)
        }
        scoReceiver = scoRcv
    }

    /**
     * Debounced connect: cancels any pending disconnect, then schedules connect after [btDebounceMs].
     * If another event arrives within the window, the previous is cancelled.
     */
    private fun debouncedConnect() {
        // Cancel opposing disconnect
        pendingDisconnectRunnable?.let { mainHandler.removeCallbacks(it) }
        pendingDisconnectRunnable = null
        // Cancel previous connect
        pendingConnectRunnable?.let { mainHandler.removeCallbacks(it) }
        val runnable = Runnable {
            Log.i(TAG, "Debounced: routing audio to Bluetooth device")
            VisioManager.onBluetoothAudioDeviceConnected()
            pendingConnectRunnable = null
        }
        pendingConnectRunnable = runnable
        mainHandler.postDelayed(runnable, btDebounceMs)
    }

    /**
     * Debounced disconnect: cancels any pending connect, then schedules disconnect after [btDebounceMs].
     */
    private fun debouncedDisconnect() {
        // Cancel opposing connect
        pendingConnectRunnable?.let { mainHandler.removeCallbacks(it) }
        pendingConnectRunnable = null
        // Cancel previous disconnect
        pendingDisconnectRunnable?.let { mainHandler.removeCallbacks(it) }
        val runnable = Runnable {
            Log.i(TAG, "Debounced: restoring default audio routing")
            VisioManager.onBluetoothAudioDeviceDisconnected()
            pendingDisconnectRunnable = null
        }
        pendingDisconnectRunnable = runnable
        mainHandler.postDelayed(runnable, btDebounceMs)
    }

    @Suppress("kotlin:S3776")
    private fun reportBluetoothCarKit() {
        val btManager = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
        val adapter = btManager?.adapter
        if (adapter == null) {
            Log.d(TAG, "No Bluetooth adapter")
            try {
                VisioManager.client.reportBluetoothCarKit(false)
            } catch (_: Exception) {
                // No-op
            }
            return
        }

        val profilesToCheck = listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET)
        val pendingProfiles = java.util.concurrent.atomic.AtomicInteger(profilesToCheck.size)
        val foundCarKit = java.util.concurrent.atomic.AtomicBoolean(false)

        try {
            for (profileType in profilesToCheck) {
                adapter.getProfileProxy(
                    context,
                    object : BluetoothProfile.ServiceListener {
                        override fun onServiceConnected(
                            profile: Int,
                            proxy: BluetoothProfile,
                        ) {
                            try {
                                val devices = proxy.connectedDevices
                                for (device in devices) {
                                    val deviceClass = device.bluetoothClass?.deviceClass ?: 0
                                    val name = device.name ?: "unknown"
                                    Log.d(
                                        TAG,
                                        "BT device: name=$name class=0x${deviceClass.toString(16)} profile=$profile",
                                    )

                                    val isWearable =
                                        deviceClass == BluetoothClass.Device.AUDIO_VIDEO_WEARABLE_HEADSET ||
                                            deviceClass == BluetoothClass.Device.AUDIO_VIDEO_HEADPHONES
                                    if (!isWearable) {
                                        foundCarKit.set(true)
                                        Log.d(TAG, "Car audio device detected: $name")
                                    }
                                }
                            } catch (e: SecurityException) {
                                Log.w(TAG, "BLUETOOTH_CONNECT permission not granted: ${e.message}")
                            } catch (_: Exception) {
                                // No-op
                            }

                            try {
                                adapter.closeProfileProxy(profile, proxy)
                            } catch (_: Exception) {
                                // No-op
                            }

                            // Only report after ALL profiles have been checked
                            if (pendingProfiles.decrementAndGet() == 0) {
                                val result = foundCarKit.get()
                                Log.d(TAG, "Bluetooth car kit (aggregated): $result")
                                try {
                                    VisioManager.client.reportBluetoothCarKit(result)
                                } catch (_: Exception) {
                                    // No-op
                                }
                            }
                        }

                        override fun onServiceDisconnected(profile: Int) {
                            // If proxy disconnects before callback, still decrement
                            if (pendingProfiles.decrementAndGet() == 0) {
                                try {
                                    VisioManager.client.reportBluetoothCarKit(foundCarKit.get())
                                } catch (_: Exception) {
                                    // No-op
                                }
                            }
                        }
                    },
                    profileType,
                )
            }
        } catch (e: SecurityException) {
            Log.w(TAG, "Missing BLUETOOTH_CONNECT permission: ${e.message}")
            try {
                VisioManager.client.reportBluetoothCarKit(false)
            } catch (_: Exception) {
                // No-op
            }
        }
    }
}
