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
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.theme.VisioColors
import uniffi.visio.Meeting
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

@Composable
fun MeetingsTab(
    meetings: List<Meeting>,
    hasCalendarUrl: Boolean,
    isLoading: Boolean,
    isDark: Boolean,
    lang: String,
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
            MeetingsList(
                meetings = meetings,
                isDark = isDark,
                lang = lang,
                onJoinMeeting = onJoinMeeting,
                onRefresh = { VisioManager.refreshCalendarNow() },
            )
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
            text = if (lang == "fr") "Connectez votre agenda" else "Connect your calendar",
            style = MaterialTheme.typography.titleLarge,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text =
                if (lang == "fr") {
                    "Ajoutez l'URL iCal de votre agenda pour voir vos réunions ici."
                } else {
                    "Add your calendar iCal URL to see your meetings here."
                },
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
                if (lang == "fr") "Configurer le calendrier" else "Configure calendar",
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
            text = if (lang == "fr") "Synchronisation du calendrier..." else "Syncing calendar...",
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
            text = "\uD83C\uDF89",
            fontSize = 64.sp,
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = if (lang == "fr") "Aucune réunion à venir" else "No upcoming meetings",
            style = MaterialTheme.typography.titleLarge,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(24.dp))
        OutlinedButton(
            onClick = onRefresh,
            shape = RoundedCornerShape(12.dp),
        ) {
            Text(if (lang == "fr") "Actualiser" else "Refresh")
        }
    }
}

@Composable
private fun MeetingsList(
    meetings: List<Meeting>,
    isDark: Boolean,
    lang: String,
    onJoinMeeting: (roomUrl: String, serverName: String) -> Unit,
    onRefresh: () -> Unit,
) {
    val now = System.currentTimeMillis() / 1000L

    // Separate in-progress meetings (started) from upcoming
    val inProgress = meetings.filter { it.startTime <= now && it.endTime > now }
    val upcoming = meetings.filter { it.startTime > now }

    // Group upcoming by day label
    val groupedUpcoming = groupMeetingsByDay(upcoming, lang)

    LazyColumn(
        modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item { Spacer(modifier = Modifier.height(8.dp)) }

        if (inProgress.isNotEmpty()) {
            item {
                Text(
                    text = if (lang == "fr") "En cours" else "In progress",
                    style = MaterialTheme.typography.labelLarge,
                    color = VisioColors.Primary500,
                    modifier = Modifier.padding(vertical = 4.dp),
                )
            }
            items(inProgress) { meeting ->
                MeetingCard(
                    meeting = meeting,
                    isDark = isDark,
                    lang = lang,
                    now = now,
                    isInProgress = true,
                    onJoin = { onJoinMeeting(meeting.roomUrl, meeting.serverName) },
                )
            }
            item { Spacer(modifier = Modifier.height(8.dp)) }
        }

        groupedUpcoming.forEach { (dayLabel, dayMeetings) ->
            item {
                Text(
                    text = dayLabel,
                    style = MaterialTheme.typography.labelLarge,
                    color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    modifier = Modifier.padding(vertical = 4.dp),
                )
            }
            items(dayMeetings) { meeting ->
                val minutesUntil = (meeting.startTime - now) / 60
                MeetingCard(
                    meeting = meeting,
                    isDark = isDark,
                    lang = lang,
                    now = now,
                    isImminent = minutesUntil < 15,
                    onJoin = { onJoinMeeting(meeting.roomUrl, meeting.serverName) },
                )
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 16.dp),
                horizontalArrangement = Arrangement.Center,
            ) {
                OutlinedButton(
                    onClick = onRefresh,
                    shape = RoundedCornerShape(12.dp),
                ) {
                    Text(if (lang == "fr") "Actualiser" else "Refresh")
                }
            }
        }
    }
}

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
    val cardColor =
        when {
            isInProgress || isImminent ->
                if (isDark) VisioColors.Primary500.copy(alpha = 0.15f) else VisioColors.Primary500.copy(alpha = 0.08f)
            else ->
                if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant
        }

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = cardColor),
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    if (isImminent && !isInProgress) {
                        PulsingDot()
                    }
                    Text(
                        text = meeting.summary,
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = FontWeight.SemiBold,
                        color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    )
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = formatMeetingTime(meeting, now, lang),
                    style = MaterialTheme.typography.bodySmall,
                    color =
                        if (isImminent || isInProgress) {
                            VisioColors.Primary500
                        } else {
                            if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary
                        },
                )
                if (meeting.serverName.isNotBlank()) {
                    Spacer(modifier = Modifier.height(2.dp))
                    Text(
                        text = meeting.serverName,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    )
                }
                if (isInProgress) {
                    Spacer(modifier = Modifier.height(2.dp))
                    Text(
                        text = if (lang == "fr") "En cours" else "In progress",
                        style = MaterialTheme.typography.labelSmall,
                        color = VisioColors.Primary500,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }
            Spacer(modifier = Modifier.width(8.dp))
            Button(
                onClick = onJoin,
                colors =
                    ButtonDefaults.buttonColors(
                        containerColor = VisioColors.Primary500,
                        contentColor = VisioColors.White,
                    ),
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(
                    if (lang == "fr") "Rejoindre" else "Join",
                    style = MaterialTheme.typography.labelLarge,
                )
            }
        }
    }
}

@Composable
private fun PulsingDot() {
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
                .background(VisioColors.Primary500, CircleShape),
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
            // in progress
            val endLocal = Instant.ofEpochSecond(meeting.endTime).atZone(zone).toLocalDateTime()
            val timeFormatter = DateTimeFormatter.ofPattern("HH:mm")
            if (lang == "fr") {
                "Jusqu'à ${timeFormatter.format(endLocal)}"
            } else {
                "Until ${timeFormatter.format(endLocal)}"
            }
        }
        minutesUntil < 60 -> {
            // relative time for imminent meetings (< 1h)
            if (lang == "fr") "Dans $minutesUntil min" else "In $minutesUntil min"
        }
        minutesUntil < 240 -> {
            // relative hours
            val hours = minutesUntil / 60
            if (lang == "fr") "Dans $hours h" else "In $hours h"
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
                    today -> if (lang == "fr") "Aujourd'hui" else "Today"
                    tomorrow -> if (lang == "fr") "Demain" else "Tomorrow"
                    else -> dayFormatter.format(date).replaceFirstChar { it.uppercase() }
                }
            label to dayMeetings.sortedBy { it.startTime }
        }
}
