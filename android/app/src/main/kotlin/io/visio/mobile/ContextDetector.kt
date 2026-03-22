package io.visio.mobile

import android.bluetooth.BluetoothClass
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
import android.util.Log
import uniffi.visio.NetworkType

class ContextDetector(private val context: Context) {
    private val connectivityManager =
        context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private val sensorManager =
        context.getSystemService(Context.SENSOR_SERVICE) as SensorManager
    private val audioManager =
        context.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var accelerometerListener: SensorEventListener? = null
    private var audioDeviceCallback: AudioDeviceCallback? = null
    private var bluetoothReceiver: android.content.BroadcastReceiver? = null

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
        networkCallback = null
        accelerometerListener = null
        audioDeviceCallback = null
        bluetoothReceiver = null
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
                    // Auto-route to newly connected Bluetooth audio device
                    val btDevice =
                        addedDevices.firstOrNull { device ->
                            device.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
                                device.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                                device.type == AudioDeviceInfo.TYPE_BLE_HEADSET
                        }
                    if (btDevice != null) {
                        Log.i(TAG, "Bluetooth audio device connected: ${btDevice.productName}, auto-routing")
                        VisioManager.onBluetoothAudioDeviceConnected()
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
                        Log.i(TAG, "Bluetooth audio device disconnected, restoring default routing")
                        VisioManager.onBluetoothAudioDeviceDisconnected()
                    }
                }
            }
        audioManager.registerAudioDeviceCallback(callback, android.os.Handler(android.os.Looper.getMainLooper()))
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
                            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                                reportBluetoothCarKit()
                            }, 500)
                            if (intent.action == android.bluetooth.BluetoothDevice.ACTION_ACL_CONNECTED) {
                                android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                                    VisioManager.onBluetoothAudioDeviceConnected()
                                }, 1000)
                            }
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
