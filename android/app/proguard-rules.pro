# Keep UniFFI generated bindings
-keep class uniffi.** { *; }

# Keep JNI native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep WebRTC classes
-keep class org.webrtc.** { *; }

# Keep LiveKit SDK classes
-keep class io.livekit.** { *; }

# Keep Compose
-dontwarn androidx.compose.**

# Suppress missing Google Error Prone annotations (used by Tink/Firebase)
-dontwarn com.google.errorprone.annotations.**

# Suppress missing javax.annotation (used by various Google libs)
-dontwarn javax.annotation.**
