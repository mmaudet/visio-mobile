package io.visio.mobile.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.visio.mobile.R
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import uniffi.visio.ParticipantInfo
import kotlin.math.absoluteValue

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ParticipantListSheet(
    participants: List<ParticipantInfo>,
    localDisplayName: String,
    localMicEnabled: Boolean,
    localCameraEnabled: Boolean,
    localIsHandRaised: Boolean,
    handRaisedMap: Map<String, Int>,
    lang: String,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = false)

    // Total count = remote participants + local user
    val totalCount = participants.size + 1

    // Sort remote participants: hand raised first (by position), then alphabetically
    val sorted = participants.sortedWith(
        compareByDescending<ParticipantInfo> { handRaisedMap[it.sid] != null }
            .thenBy { handRaisedMap[it.sid] ?: Int.MAX_VALUE }
            .thenBy { (it.name ?: it.identity).lowercase() }
    )

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = VisioColors.PrimaryDark75
    ) {
        // Header
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "${Strings.t("participants.title", lang)} ($totalCount)",
                color = VisioColors.White,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold
            )
            IconButton(onClick = onDismiss, modifier = Modifier.size(32.dp)) {
                Icon(
                    painter = painterResource(R.drawable.ri_close_line),
                    contentDescription = Strings.t("participants.close", lang),
                    tint = VisioColors.White,
                    modifier = Modifier.size(20.dp)
                )
            }
        }

        // Participant list
        LazyColumn(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            // Local participant (always first)
            item(key = "local") {
                val name = localDisplayName.ifBlank { Strings.t("call.you", lang) }
                ParticipantRow(
                    name = name,
                    suffix = "(${Strings.t("call.you", lang)})",
                    isMuted = !localMicEnabled,
                    hasVideo = localCameraEnabled,
                    handRaisePosition = if (localIsHandRaised) 1 else 0,
                    qualityName = "Excellent"
                )
            }

            // Remote participants
            items(sorted, key = { it.sid }) { p ->
                ParticipantRow(
                    name = p.name ?: p.identity,
                    suffix = null,
                    isMuted = p.isMuted,
                    hasVideo = p.hasVideo,
                    handRaisePosition = handRaisedMap[p.sid] ?: 0,
                    qualityName = p.connectionQuality.name
                )
            }
        }

        Spacer(modifier = Modifier.height(32.dp))
    }
}

@Composable
private fun ParticipantRow(
    name: String,
    suffix: String?,
    isMuted: Boolean,
    hasVideo: Boolean,
    handRaisePosition: Int,
    qualityName: String
) {
    val initials = name
        .split(" ")
        .mapNotNull { it.firstOrNull()?.uppercase() }
        .take(2)
        .joinToString("")
        .ifEmpty { "?" }

    val hue = name.fold(0) { acc, c -> acc + c.code }.absoluteValue % 360
    val avatarColor = Color.hsl(hue.toFloat(), 0.5f, 0.35f)

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Avatar
        Box(
            modifier = Modifier
                .size(40.dp)
                .clip(CircleShape)
                .background(avatarColor),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = initials,
                color = VisioColors.White,
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold
            )
        }

        // Name + suffix
        Row(
            modifier = Modifier.weight(1f),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Text(
                text = name,
                color = VisioColors.White,
                fontSize = 15.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false)
            )
            if (suffix != null) {
                Text(
                    text = suffix,
                    color = VisioColors.Greyscale400,
                    fontSize = 13.sp,
                    maxLines = 1
                )
            }
        }

        // Status icons
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            // Mic muted
            if (isMuted) {
                Icon(
                    painter = painterResource(R.drawable.ri_mic_off_fill),
                    contentDescription = null,
                    tint = VisioColors.Error500,
                    modifier = Modifier.size(16.dp)
                )
            }

            // Camera off
            if (!hasVideo) {
                Icon(
                    painter = painterResource(R.drawable.ri_video_off_fill),
                    contentDescription = null,
                    tint = VisioColors.Error500,
                    modifier = Modifier.size(16.dp)
                )
            }

            // Hand raised
            if (handRaisePosition > 0) {
                Row(
                    modifier = Modifier
                        .background(VisioColors.HandRaise, RoundedCornerShape(10.dp))
                        .padding(horizontal = 5.dp, vertical = 1.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(2.dp)
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ri_hand),
                        contentDescription = null,
                        tint = Color.Black,
                        modifier = Modifier.size(12.dp)
                    )
                    Text(
                        text = "$handRaisePosition",
                        color = Color.Black,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold
                    )
                }
            }

            // Connection quality
            ConnectionQualityBarsSmall(qualityName)
        }
    }
}

@Composable
private fun ConnectionQualityBarsSmall(quality: String) {
    val bars = when (quality) {
        "Excellent" -> 3
        "Good" -> 2
        "Poor" -> 1
        else -> 0
    }
    Row(
        horizontalArrangement = Arrangement.spacedBy(1.dp),
        verticalAlignment = Alignment.Bottom
    ) {
        for (i in 1..3) {
            Box(
                modifier = Modifier
                    .width(3.dp)
                    .height((i * 3 + 2).dp)
                    .background(
                        if (i <= bars) Color.Green else VisioColors.Greyscale400,
                        RoundedCornerShape(1.dp)
                    )
            )
        }
    }
}
