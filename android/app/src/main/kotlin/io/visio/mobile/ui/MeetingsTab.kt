package io.visio.mobile.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import uniffi.visio.Meeting
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MeetingsTab(
    meetings: List<Meeting>,
    hasCalendarUrl: Boolean,
    isLoading: Boolean,
    isDark: Boolean,
    lang: String,
    nowSeconds: Long = System.currentTimeMillis() / 1000L,
    onSettings: () -> Unit,
    onJoinMeeting: (roomUrl: String, serverName: String) -> Unit,
) {
    when {
        !hasCalendarUrl -> {
            MeetingsOnboarding(isDark = isDark, lang = lang, onSettings = onSettings)
        }
        isLoading && meetings.isEmpty() -> {
            MeetingsLoading(isDark = isDark, lang = lang)
        }
        meetings.isEmpty() -> {
            MeetingsEmpty(isDark = isDark, lang = lang, onRefresh = { VisioManager.refreshCalendarNow() })
        }
        else -> {
            PullToRefreshBox(
                isRefreshing = isLoading,
                onRefresh = { VisioManager.refreshCalendarNow() },
            ) {
                MeetingsList(
                    meetings = meetings,
                    isDark = isDark,
                    lang = lang,
                    nowSeconds = nowSeconds,
                    onJoinMeeting = onJoinMeeting,
                )
            }
        }
    }
}

@Composable
private fun MeetingsOnboarding(
    isDark: Boolean,
    lang: String,
    onSettings: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "\uD83D\uDCC5",
            fontSize = 64.sp,
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = Strings.t("meetings.onboarding.title", lang),
            style = MaterialTheme.typography.titleLarge,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = Strings.t("meetings.onboarding", lang),
            style = MaterialTheme.typography.bodyMedium,
            color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(24.dp))
        Button(
            onClick = onSettings,
            colors =
                ButtonDefaults.buttonColors(
                    containerColor = VisioColors.Primary500,
                    contentColor = VisioColors.White,
                ),
            shape = RoundedCornerShape(12.dp),
        ) {
            Text(
                Strings.t("meetings.onboarding.configure", lang),
                modifier = Modifier.padding(vertical = 4.dp),
            )
        }
    }
}

@Composable
private fun MeetingsLoading(
    isDark: Boolean,
    lang: String,
) {
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        CircularProgressIndicator(color = VisioColors.Primary500)
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = Strings.t("meetings.loading", lang),
            style = MaterialTheme.typography.bodyMedium,
            color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
        )
    }
}

@Composable
private fun MeetingsEmpty(
    isDark: Boolean,
    lang: String,
    onRefresh: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "\u2600\uFE0F",
            fontSize = 64.sp,
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = Strings.t("meetings.empty", lang),
            style = MaterialTheme.typography.titleLarge,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = Strings.t("meetings.empty.subtitle", lang),
            style = MaterialTheme.typography.bodyMedium,
            color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
            textAlign = TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(24.dp))
        OutlinedButton(
            onClick = onRefresh,
            shape = RoundedCornerShape(12.dp),
        ) {
            Text(Strings.t("meetings.refresh", lang))
        }
    }
}

@Composable
private fun MeetingsList(
    meetings: List<Meeting>,
    isDark: Boolean,
    lang: String,
    nowSeconds: Long = System.currentTimeMillis() / 1000L,
    onJoinMeeting: (roomUrl: String, serverName: String) -> Unit,
) {
    val now = nowSeconds

    // Separate in-progress meetings (started) from upcoming
    val inProgress = meetings.filter { it.startTime <= now && it.endTime > now }
    val upcoming = meetings.filter { it.startTime > now }

    // Combine in-progress and upcoming into a unified grouped list
    val allMeetings = inProgress + upcoming
    val allGrouped = groupMeetingsByDay(allMeetings, lang)

    LazyColumn(
        modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item { Spacer(modifier = Modifier.height(8.dp)) }

        allGrouped.forEach { (dayLabel, dayMeetings) ->
            item {
                Text(
                    text = dayLabel.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.SemiBold,
                    letterSpacing = 1.sp,
                    color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
                )
            }
            items(dayMeetings) { meeting ->
                val minutesUntil = (meeting.startTime - now) / 60
                val isInProgressItem = meeting.startTime <= now && meeting.endTime > now
                MeetingCard(
                    meeting = meeting,
                    isDark = isDark,
                    lang = lang,
                    now = now,
                    isInProgress = isInProgressItem,
                    isImminent = !isInProgressItem && minutesUntil in 0..14,
                    onJoin = { onJoinMeeting(meeting.roomUrl, meeting.serverName) },
                )
            }
        }

        item {
            Text(
                text = Strings.t("meetings.sync", lang).replace("{time}", "< 1 min"),
                style = MaterialTheme.typography.bodySmall,
                color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp),
            )
        }
    }
}

@Suppress("kotlin:S107")
@Composable
private fun MeetingCard(
    meeting: Meeting,
    isDark: Boolean,
    lang: String,
    now: Long,
    isInProgress: Boolean = false,
    isImminent: Boolean = false,
    onJoin: () -> Unit,
) {
    val isAccent = isInProgress || isImminent
    val accentGradient =
        Brush.linearGradient(
            colors = listOf(VisioColors.Primary500, Color(0xFF5C3CDC)),
        )

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(10.dp),
        colors =
            CardDefaults.cardColors(
                containerColor =
                    if (isAccent) {
                        Color.Transparent
                    } else if (isDark) {
                        VisioColors.PrimaryDark100
                    } else {
                        VisioColors.LightSurfaceVariant
                    },
            ),
    ) {
        Row(
            modifier =
                Modifier
                    .then(
                        if (isAccent) Modifier.background(accentGradient, RoundedCornerShape(10.dp)) else Modifier,
                    )
                    .fillMaxWidth()
                    .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            MeetingCardDetails(
                meeting = meeting,
                isDark = isDark,
                lang = lang,
                now = now,
                isAccent = isAccent,
                isImminent = isImminent,
                isInProgress = isInProgress,
                modifier = Modifier.weight(1f),
            )
            Spacer(modifier = Modifier.width(8.dp))
            OutlinedButton(
                onClick = onJoin,
                shape = RoundedCornerShape(8.dp),
                colors =
                    ButtonDefaults.outlinedButtonColors(
                        contentColor = if (isAccent) VisioColors.White else VisioColors.Primary500,
                    ),
                border =
                    androidx.compose.foundation.BorderStroke(
                        1.5.dp,
                        if (isAccent) VisioColors.White else VisioColors.Primary500,
                    ),
            ) {
                Text(
                    Strings.t("home.tab.join", lang),
                    style = MaterialTheme.typography.labelLarge,
                    fontWeight = FontWeight.SemiBold,
                )
            }
        }
    }
}

@Suppress("kotlin:S107")
@Composable
private fun MeetingCardDetails(
    meeting: Meeting,
    isDark: Boolean,
    lang: String,
    now: Long,
    isAccent: Boolean,
    isImminent: Boolean,
    isInProgress: Boolean,
    modifier: Modifier = Modifier,
) {
    val primaryColor =
        if (isAccent) {
            VisioColors.White
        } else if (isDark) {
            VisioColors.White
        } else {
            VisioColors.LightOnBackground
        }

    val secondaryColor =
        if (isAccent) {
            VisioColors.White.copy(alpha = 0.8f)
        } else if (isDark) {
            VisioColors.Greyscale400
        } else {
            VisioColors.LightTextSecondary
        }

    Column(modifier = modifier) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            if (isImminent || isInProgress) {
                PulsingDot(
                    color = if (isInProgress) VisioColors.Error500 else Color(0xFF4ADE80),
                )
            }
            Text(
                text = meeting.summary,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Bold,
                color = primaryColor,
            )
        }
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = formatMeetingTime(meeting, now, lang),
            style = MaterialTheme.typography.bodySmall,
            color = secondaryColor,
        )
        if (meeting.serverName.isNotBlank()) {
            Spacer(modifier = Modifier.height(2.dp))
            Text(
                text = meeting.serverName,
                style = MaterialTheme.typography.bodySmall,
                color = secondaryColor,
            )
        }
    }
}

@Composable
private fun PulsingDot(color: Color = Color(0xFF4ADE80)) {
    val infiniteTransition = rememberInfiniteTransition(label = "pulse")
    val alpha by
        infiniteTransition.animateFloat(
            initialValue = 1f,
            targetValue = 0.3f,
            animationSpec =
                infiniteRepeatable(
                    animation = tween(800),
                    repeatMode = RepeatMode.Reverse,
                ),
            label = "dot_alpha",
        )
    Box(
        modifier =
            Modifier
                .size(8.dp)
                .alpha(alpha)
                .background(color, CircleShape),
    )
}

private fun formatMeetingTime(
    meeting: Meeting,
    nowSeconds: Long,
    lang: String,
): String {
    val minutesUntil = (meeting.startTime - nowSeconds) / 60
    val zone = ZoneId.systemDefault()
    val startInstant = Instant.ofEpochSecond(meeting.startTime)
    val startLocal = startInstant.atZone(zone).toLocalDateTime()
    val nowLocal = Instant.ofEpochSecond(nowSeconds).atZone(zone).toLocalDate()

    return when {
        minutesUntil < 0 -> {
            // in progress — show "En cours" + until time
            val endLocal = Instant.ofEpochSecond(meeting.endTime).atZone(zone).toLocalDateTime()
            val timeFormatter = DateTimeFormatter.ofPattern("HH:mm")
            val untilStr =
                Strings.t("meetings.time.until", lang)
                    .replace("{time}", timeFormatter.format(endLocal))
            "${Strings.t("meetings.time.inProgress", lang)} · $untilStr"
        }
        minutesUntil < 60 -> {
            // relative time for imminent meetings (< 1h)
            Strings.t("meetings.time.inMinutes", lang).replace("{minutes}", minutesUntil.toString())
        }
        minutesUntil < 240 -> {
            // relative hours + minutes
            val hours = minutesUntil / 60
            val mins = minutesUntil % 60
            if (mins > 0) {
                Strings.t("meetings.time.inHoursMinutes", lang)
                    .replace("{hours}", hours.toString())
                    .replace("{minutes}", mins.toString().padStart(2, '0'))
            } else {
                Strings.t("meetings.time.inHours", lang).replace("{hours}", hours.toString())
            }
        }
        startLocal.toLocalDate() == nowLocal -> {
            // same day: show time
            val timeFormatter = DateTimeFormatter.ofPattern("HH:mm")
            timeFormatter.format(startLocal)
        }
        else -> {
            // different day: show day + time
            val locale = if (lang == "fr") Locale.FRENCH else Locale.ENGLISH
            val formatter = DateTimeFormatter.ofPattern("EEE HH:mm", locale)
            formatter.format(startLocal)
        }
    }
}

private fun groupMeetingsByDay(
    meetings: List<Meeting>,
    lang: String,
): List<Pair<String, List<Meeting>>> {
    if (meetings.isEmpty()) return emptyList()

    val zone = ZoneId.systemDefault()
    val today = java.time.LocalDate.now(zone)
    val tomorrow = today.plusDays(1)
    val locale = if (lang == "fr") Locale.FRENCH else Locale.ENGLISH
    val dayFormatter = DateTimeFormatter.ofPattern("EEEE d MMMM", locale)

    return meetings
        .groupBy { meeting ->
            Instant.ofEpochSecond(meeting.startTime).atZone(zone).toLocalDate()
        }
        .entries
        .sortedBy { it.key }
        .map { (date, dayMeetings) ->
            val label =
                when (date) {
                    today -> Strings.t("meetings.today", lang)
                    tomorrow -> Strings.t("meetings.tomorrow", lang)
                    else -> dayFormatter.format(date).replaceFirstChar { it.uppercase() }
                }
            label.uppercase() to dayMeetings.sortedBy { it.startTime }
        }
}
