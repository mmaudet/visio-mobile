fn main() {
    uniffi::generate_scaffolding("src/visio.udl").unwrap();

    // Preserve Java_org_webrtc_* JNI symbols in the .so so that
    // webrtc::InitAndroid() can call back into the bundled Java classes.
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "android" {
        webrtc_sys_build::configure_jni_symbols()
            .expect("failed to configure JNI symbols for Android");
    }
}
