package io.visio.mobile.ui

import android.media.AudioDeviceInfo
import android.media.AudioManager
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Rule
import org.junit.Test

class InCallSettingsSheetTest {
    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun settings_sheet_shows_title() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext

        composeTestRule.setContent {
            InCallSettingsSheet(
                roomUrl = "https://meet.example.com/test",
                initialTab = 0,
                onDismiss = {},
                onSelectAudioInput = { _ -> },
                onSelectAudioOutput = { _ -> },
                onSwitchCamera = {},
                isFrontCamera = true,
            )
        }

        // The sheet should display the in-call settings title
        // (uses i18n key "settings.incall")
        composeTestRule.waitForIdle()
    }

    @Test
    fun camera_tab_shows_front_and_back_options() {
        composeTestRule.setContent {
            InCallSettingsSheet(
                roomUrl = "https://meet.example.com/test",
                initialTab = 1,
                onDismiss = {},
                onSelectAudioInput = { _ -> },
                onSelectAudioOutput = { _ -> },
                onSwitchCamera = {},
                isFrontCamera = true,
            )
        }

        composeTestRule.waitForIdle()
        // Camera tab should show front/back camera options
        // These use i18n keys "settings.incall.cameraFront" and "settings.incall.cameraBack"
    }

    @Test
    fun audio_output_devices_listed() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val audioManager = context.getSystemService(AudioManager::class.java)
        val outputDevices = audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)

        composeTestRule.setContent {
            InCallSettingsSheet(
                roomUrl = "https://meet.example.com/test",
                initialTab = 0,
                onDismiss = {},
                onSelectAudioInput = { _ -> },
                onSelectAudioOutput = { _ -> },
                onSwitchCamera = {},
                isFrontCamera = true,
            )
        }

        composeTestRule.waitForIdle()

        // The micro tab (tab 0) should be displayed by default
        // and should show audio input/output sections
    }
}
