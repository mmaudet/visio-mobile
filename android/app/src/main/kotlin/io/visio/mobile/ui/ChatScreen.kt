package io.visio.mobile.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(
    onBack: () -> Unit
) {
    val messages by VisioManager.chatMessages.collectAsState()
    var inputText by remember { mutableStateOf("") }
    val listState = rememberLazyListState()
    val lang = VisioManager.currentLang

    // Mark chat as open when entering, closed when leaving
    LaunchedEffect(Unit) {
        try { VisioManager.client.setChatOpen(true) } catch (_: Exception) {}
    }
    DisposableEffect(Unit) {
        onDispose {
            try { VisioManager.client.setChatOpen(false) } catch (_: Exception) {}
        }
    }

    // Scroll to bottom when new messages arrive
    LaunchedEffect(messages.size) {
        if (messages.isNotEmpty()) {
            listState.animateScrollToItem(messages.size - 1)
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(VisioColors.PrimaryDark50)
            .imePadding()
    ) {
        // Top bar
        TopAppBar(
            title = {
                Text(Strings.t("chat", lang), color = VisioColors.White)
            },
            navigationIcon = {
                IconButton(onClick = onBack) {
                    Icon(
                        painter = painterResource(R.drawable.ri_arrow_left_s_line),
                        contentDescription = Strings.t("accessibility.back", lang),
                        tint = VisioColors.White
                    )
                }
            },
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor = VisioColors.PrimaryDark75
            )
        )

        // Messages list
        LazyColumn(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(horizontal = 12.dp),
            state = listState,
            verticalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            itemsIndexed(messages, key = { _, msg -> msg.id }) { index, message ->
                val isOwn = message.senderSid == "local" ||
                    message.senderName == (try { VisioManager.client.getSettings().displayName } catch (_: Exception) { null })

                // Show sender name if first message or different sender from previous
                val showSender = index == 0 ||
                    messages[index - 1].senderSid != message.senderSid ||
                    (message.timestampMs.toLong() - messages[index - 1].timestampMs.toLong()) > 60_000

                ChatBubble(
                    message = message,
                    isOwn = isOwn,
                    showSender = showSender
                )
            }
        }

        // Input bar
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(VisioColors.PrimaryDark75)
                .padding(8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            TextField(
                value = inputText,
                onValueChange = { inputText = it },
                placeholder = {
                    Text("Type a message", color = VisioColors.Greyscale400)
                },
                modifier = Modifier.weight(1f),
                singleLine = true,
                colors = TextFieldDefaults.colors(
                    focusedContainerColor = VisioColors.PrimaryDark100,
                    unfocusedContainerColor = VisioColors.PrimaryDark100,
                    cursorColor = VisioColors.Primary500,
                    focusedTextColor = VisioColors.White,
                    unfocusedTextColor = VisioColors.White,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent
                ),
                shape = RoundedCornerShape(12.dp)
            )
            IconButton(
                onClick = {
                    val text = inputText.trim()
                    if (text.isNotEmpty()) {
                        try {
                            VisioManager.client.sendChatMessage(text)
                            inputText = ""
                        } catch (_: Exception) {}
                    }
                },
                enabled = inputText.isNotBlank()
            ) {
                Icon(
                    painter = painterResource(R.drawable.ri_send_plane_2_fill),
                    contentDescription = Strings.t("accessibility.send", lang),
                    tint = if (inputText.isNotBlank()) VisioColors.Primary500 else VisioColors.Greyscale400,
                    modifier = Modifier.size(24.dp)
                )
            }
        }
    }
}

@Composable
private fun ChatBubble(
    message: uniffi.visio.ChatMessage,
    isOwn: Boolean,
    showSender: Boolean
) {
    val timeFormat = remember { SimpleDateFormat("HH:mm", Locale.getDefault()) }
    val timestamp = remember(message.timestampMs) {
        timeFormat.format(Date(message.timestampMs.toLong()))
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 2.dp),
        horizontalAlignment = if (isOwn) Alignment.End else Alignment.Start
    ) {
        // Sender name + timestamp
        if (showSender) {
            Spacer(modifier = Modifier.height(8.dp))
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                if (!isOwn) {
                    Text(
                        text = message.senderName,
                        style = MaterialTheme.typography.labelSmall,
                        color = VisioColors.Primary500,
                        fontWeight = FontWeight.SemiBold
                    )
                }
                Text(
                    text = timestamp,
                    style = MaterialTheme.typography.labelSmall,
                    color = VisioColors.Greyscale400
                )
            }
            Spacer(modifier = Modifier.height(2.dp))
        }

        // Bubble
        Box(
            modifier = Modifier
                .widthIn(max = 280.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(
                    if (isOwn) VisioColors.Primary500 else VisioColors.PrimaryDark100
                )
                .padding(horizontal = 12.dp, vertical = 8.dp)
        ) {
            Text(
                text = message.text,
                style = MaterialTheme.typography.bodyMedium,
                color = VisioColors.White,
                fontSize = 14.sp
            )
        }
    }
}
