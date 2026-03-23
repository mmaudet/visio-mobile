# Keep UniFFI generated bindings
-keep class uniffi.** { *; }

# Keep JNA classes (UniFFI uses JNA; fields accessed via reflection/JNI)
-keep class com.sun.jna.** { *; }
-dontwarn java.awt.**

# Keep JNI native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep WebRTC classes and jni_zero (used by native WebRTC code via FindClass)
-keep class org.webrtc.** { *; }
-keep class org.jni_zero.** { *; }

# Keep LiveKit SDK classes
-keep class io.livekit.** { *; }

# Keep app classes used by JNI/native code
-keep class io.visio.mobile.** { *; }

# Keep Compose
-dontwarn androidx.compose.**

# Suppress missing Google Error Prone annotations (used by Tink/Firebase)
-dontwarn com.google.errorprone.annotations.**

# Suppress missing javax.annotation (used by various Google libs)
-dontwarn javax.annotation.**
