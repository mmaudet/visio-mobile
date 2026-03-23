# Keep UniFFI generated bindings
-keep class uniffi.** { *; }

# Keep JNI native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep WebRTC classes and jni_zero (used by native WebRTC code via FindClass)
-keep class org.webrtc.** { *; }
-keep class org.jni_zero.** { *; }

# Keep LiveKit SDK classes
-keep class io.livekit.** { *; }

# Keep Compose
-dontwarn androidx.compose.**

# Suppress missing Google Error Prone annotations (used by Tink/Firebase)
-dontwarn com.google.errorprone.annotations.**

# Suppress missing javax.annotation (used by various Google libs)
-dontwarn javax.annotation.**
