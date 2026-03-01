package io.visio.mobile.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import uniffi.visio.RoomValidationResult

@Composable
fun HomeScreen(
    onJoin: (roomUrl: String, username: String) -> Unit,
    onSettings: () -> Unit
) {
    var roomUrl by remember { mutableStateOf("https://meet.example.com/room-name") }
    var username by remember { mutableStateOf("") }
    val lang = VisioManager.currentLang
    val isDark = VisioManager.currentTheme == "dark"
    var roomStatus by remember { mutableStateOf("idle") }
    val slugRegex = remember { Regex("^[a-z]{3}-[a-z]{4}-[a-z]{3}$") }

    LaunchedEffect(roomUrl) {
        val trimmed = roomUrl.trim().trimEnd('/')
        val candidate = if ('/' in trimmed) trimmed.substringAfterLast('/') else trimmed
        if (!slugRegex.matches(candidate)) {
            roomStatus = "idle"
            return@LaunchedEffect
        }
        roomStatus = "checking"
        delay(500)
        try {
            val result = withContext(Dispatchers.IO) {
                VisioManager.client.validateRoom(roomUrl.trim(), username.trim().ifEmpty { null })
            }
            roomStatus = when (result) {
                is RoomValidationResult.Valid -> "valid"
                is RoomValidationResult.NotFound -> "not_found"
                is RoomValidationResult.InvalidFormat -> "idle"
                is RoomValidationResult.NetworkError -> "error"
            }
        } catch (_: Exception) {
            roomStatus = "error"
        }
    }

    // Pre-fill display name from VisioManager observable state
    LaunchedEffect(Unit) {
        val name = VisioManager.displayName
        if (name.isNotBlank() && username.isEmpty()) {
            username = name
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .statusBarsPadding()
            .navigationBarsPadding()
            .padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        // Title row with settings gear
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Spacer(modifier = Modifier.size(48.dp)) // balance the gear icon
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                VisioLogo(size = 64.dp)
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = Strings.t("app.title", lang),
                    style = MaterialTheme.typography.headlineLarge,
                    color = MaterialTheme.colorScheme.onBackground,
                    fontWeight = FontWeight.Bold
                )
            }
            IconButton(
                onClick = onSettings,
                modifier = Modifier.size(48.dp)
            ) {
                Icon(
                    painter = painterResource(R.drawable.ri_settings_3_line),
                    contentDescription = Strings.t("settings", lang),
                    tint = if (isDark) VisioColors.White else VisioColors.Greyscale400,
                    modifier = Modifier.size(24.dp)
                )
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = Strings.t("home.subtitle", lang),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onBackground.copy(alpha = 0.7f)
        )

        Spacer(modifier = Modifier.height(24.dp))

        TextField(
            value = roomUrl,
            onValueChange = { roomUrl = it },
            label = { Text(Strings.t("home.meetUrl", lang), color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary) },
            placeholder = { Text(Strings.t("home.meetUrl.placeholder", lang), color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                cursorColor = VisioColors.Primary500,
                focusedTextColor = MaterialTheme.colorScheme.onSurface,
                unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                focusedLabelColor = VisioColors.Primary500,
                unfocusedLabelColor = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent
            ),
            shape = RoundedCornerShape(12.dp)
        )

        when (roomStatus) {
            "checking" -> Text(
                Strings.t("home.room.checking", lang),
                style = MaterialTheme.typography.bodySmall,
                color = VisioColors.Greyscale400,
                modifier = Modifier.fillMaxWidth().padding(top = 4.dp)
            )
            "valid" -> Text(
                Strings.t("home.room.valid", lang),
                style = MaterialTheme.typography.bodySmall,
                color = Color(0xFF18753C),
                modifier = Modifier.fillMaxWidth().padding(top = 4.dp)
            )
            "not_found" -> Text(
                Strings.t("home.room.notFound", lang),
                style = MaterialTheme.typography.bodySmall,
                color = Color(0xFFE1000F),
                modifier = Modifier.fillMaxWidth().padding(top = 4.dp)
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        TextField(
            value = username,
            onValueChange = { username = it },
            label = { Text(Strings.t("home.displayName", lang), color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary) },
            placeholder = { Text(Strings.t("home.displayName.placeholder", lang), color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                cursorColor = VisioColors.Primary500,
                focusedTextColor = MaterialTheme.colorScheme.onSurface,
                unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                focusedLabelColor = VisioColors.Primary500,
                unfocusedLabelColor = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent
            ),
            shape = RoundedCornerShape(12.dp)
        )

        Spacer(modifier = Modifier.height(24.dp))

        Button(
            onClick = { onJoin(roomUrl.trim(), username.trim()) },
            enabled = roomStatus == "valid",
            modifier = Modifier.fillMaxWidth(),
            colors = ButtonDefaults.buttonColors(
                containerColor = VisioColors.Primary500,
                contentColor = VisioColors.White,
                disabledContainerColor = VisioColors.PrimaryDark300,
                disabledContentColor = VisioColors.Greyscale400
            ),
            shape = RoundedCornerShape(12.dp)
        ) {
            Text(
                Strings.t("home.join", lang),
                fontSize = 16.sp,
                modifier = Modifier.padding(vertical = 4.dp)
            )
        }
    }
}
