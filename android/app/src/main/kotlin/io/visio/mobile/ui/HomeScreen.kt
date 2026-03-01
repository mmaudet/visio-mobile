package io.visio.mobile.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
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
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors

@Composable
fun HomeScreen(
    onJoin: (roomUrl: String, username: String) -> Unit,
    onSettings: () -> Unit
) {
    var roomUrl by remember { mutableStateOf("https://meet.example.com/room-name") }
    var username by remember { mutableStateOf("") }
    var lang by remember { mutableStateOf(Strings.detectSystemLang()) }

    // Pre-fill display name and language from settings
    LaunchedEffect(Unit) {
        try {
            val settings = VisioManager.client.getSettings()
            if (!settings.displayName.isNullOrBlank()) {
                username = settings.displayName!!
            }
            if (!settings.language.isNullOrBlank()) {
                lang = settings.language!!
            }
        } catch (_: Exception) {}
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(VisioColors.PrimaryDark50)
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
            Text(
                text = Strings.t("app.title", lang),
                style = MaterialTheme.typography.headlineLarge,
                color = VisioColors.White,
                fontWeight = FontWeight.Bold
            )
            IconButton(
                onClick = onSettings,
                modifier = Modifier.size(48.dp)
            ) {
                Icon(
                    painter = painterResource(R.drawable.ri_settings_3_line),
                    contentDescription = "Settings",
                    tint = VisioColors.White,
                    modifier = Modifier.size(24.dp)
                )
            }
        }

        Spacer(modifier = Modifier.height(32.dp))

        TextField(
            value = roomUrl,
            onValueChange = { roomUrl = it },
            label = { Text("Room URL", color = VisioColors.Greyscale400) },
            placeholder = { Text("meet.example.com/room-name", color = VisioColors.Greyscale400) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = VisioColors.PrimaryDark100,
                unfocusedContainerColor = VisioColors.PrimaryDark100,
                cursorColor = VisioColors.Primary500,
                focusedTextColor = VisioColors.White,
                unfocusedTextColor = VisioColors.White,
                focusedLabelColor = VisioColors.Primary500,
                unfocusedLabelColor = VisioColors.Greyscale400,
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent
            ),
            shape = RoundedCornerShape(12.dp)
        )

        Spacer(modifier = Modifier.height(16.dp))

        TextField(
            value = username,
            onValueChange = { username = it },
            label = { Text("Display name (optional)", color = VisioColors.Greyscale400) },
            placeholder = { Text("Your name", color = VisioColors.Greyscale400) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = VisioColors.PrimaryDark100,
                unfocusedContainerColor = VisioColors.PrimaryDark100,
                cursorColor = VisioColors.Primary500,
                focusedTextColor = VisioColors.White,
                unfocusedTextColor = VisioColors.White,
                focusedLabelColor = VisioColors.Primary500,
                unfocusedLabelColor = VisioColors.Greyscale400,
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent
            ),
            shape = RoundedCornerShape(12.dp)
        )

        Spacer(modifier = Modifier.height(24.dp))

        Button(
            onClick = { onJoin(roomUrl.trim(), username.trim()) },
            enabled = roomUrl.isNotBlank(),
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
                "Join",
                fontSize = 16.sp,
                modifier = Modifier.padding(vertical = 4.dp)
            )
        }
    }
}
