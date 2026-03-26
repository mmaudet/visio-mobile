package io.visio.mobile.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import org.junit.Rule
import org.junit.Test
import uniffi.visio.ConnectionQuality
import uniffi.visio.ParticipantInfo

class ParticipantTileTest {
    @get:Rule
    val composeTestRule = createComposeRule()

    private fun makeParticipant(
        sid: String = "PA_test",
        identity: String = "test-user",
        name: String? = "Test User",
        isMuted: Boolean = false,
        hasVideo: Boolean = false,
        videoTrackSid: String? = null,
        hasScreenShare: Boolean = false,
        screenShareTrackSid: String? = null,
        color: String? = null,
        isAdmin: Boolean = false,
    ) = ParticipantInfo(
        sid = sid,
        identity = identity,
        name = name,
        isMuted = isMuted,
        hasVideo = hasVideo,
        videoTrackSid = videoTrackSid,
        hasScreenShare = hasScreenShare,
        screenShareTrackSid = screenShareTrackSid,
        connectionQuality = ConnectionQuality.GOOD,
        color = color,
        isAdmin = isAdmin,
    )

    @Test
    fun avatar_shown_when_no_video() {
        val participant = makeParticipant(name = "Alice Bob")

        composeTestRule.setContent {
            ParticipantTile(
                participant = participant,
                isActiveSpeaker = false,
                handRaisePosition = 0,
                onClick = {},
            )
        }

        // Initials "AB" should be displayed as avatar fallback
        composeTestRule.onNodeWithText("AB").assertIsDisplayed()
    }

    @Test
    fun name_shown_in_metadata_bar() {
        val participant = makeParticipant(name = "Alice Bob")

        composeTestRule.setContent {
            ParticipantTile(
                participant = participant,
                isActiveSpeaker = false,
                handRaisePosition = 0,
                onClick = {},
            )
        }

        composeTestRule.onNodeWithText("Alice Bob").assertIsDisplayed()
    }

    @Test
    fun muted_icon_shown() {
        val participant = makeParticipant(isMuted = true)

        composeTestRule.setContent {
            ParticipantTile(
                participant = participant,
                isActiveSpeaker = false,
                handRaisePosition = 0,
                onClick = {},
            )
        }

        // The muted icon uses accessibility.muted content description
        // Check that the muted indicator (mic off icon) is displayed
        composeTestRule
            .onNodeWithContentDescription("Micro coupé", substring = true, useUnmergedTree = true)
            .assertExists()
    }

    @Test
    fun hand_raise_badge_shown() {
        val participant = makeParticipant()

        composeTestRule.setContent {
            ParticipantTile(
                participant = participant,
                isActiveSpeaker = false,
                handRaisePosition = 3,
                onClick = {},
            )
        }

        // Hand raise position number should be visible
        composeTestRule.onNodeWithText("3").assertIsDisplayed()
    }
}
