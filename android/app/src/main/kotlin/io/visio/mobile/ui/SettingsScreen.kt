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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit
) {
    var displayName by remember { mutableStateOf("") }
    var language by remember { mutableStateOf(Strings.detectSystemLang()) }
    var micOnJoin by remember { mutableStateOf(true) }
    var cameraOnJoin by remember { mutableStateOf(false) }
    val coroutineScope = rememberCoroutineScope()

    // Load current settings
    LaunchedEffect(Unit) {
        try {
            val settings = VisioManager.client.getSettings()
            displayName = settings.displayName ?: ""
            language = settings.language ?: Strings.detectSystemLang()
            micOnJoin = settings.micEnabledOnJoin
            cameraOnJoin = settings.cameraEnabledOnJoin
        } catch (_: Exception) {}
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(VisioColors.PrimaryDark50)
    ) {
        TopAppBar(
            title = {
                Text("Settings", color = VisioColors.White)
            },
            navigationIcon = {
                IconButton(onClick = onBack) {
                    Icon(
                        painter = painterResource(R.drawable.ri_arrow_left_s_line),
                        contentDescription = "Back",
                        tint = VisioColors.White
                    )
                }
            },
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor = VisioColors.PrimaryDark75
            )
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp)
        ) {
            // Profile section
            SectionHeader("Profile")
            Text(
                text = "Display name",
                style = MaterialTheme.typography.bodyMedium,
                color = VisioColors.Greyscale400
            )
            TextField(
                value = displayName,
                onValueChange = { displayName = it },
                placeholder = { Text("Your name", color = VisioColors.Greyscale400) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
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

            // Join meeting section
            SectionHeader("Join meeting")
            SettingsToggle(
                label = "Mic enabled on join",
                checked = micOnJoin,
                onCheckedChange = { micOnJoin = it }
            )
            SettingsToggle(
                label = "Camera enabled on join",
                checked = cameraOnJoin,
                onCheckedChange = { cameraOnJoin = it }
            )

            // Language section
            SectionHeader(Strings.t("settings.language", language))
            Strings.supportedLangs.forEach { code ->
                LanguageOption(
                    label = Strings.t("lang.$code", code),
                    value = code,
                    selected = language,
                    onSelect = { language = it }
                )
            }
        }

        // Save button
        Button(
            onClick = {
                coroutineScope.launch(Dispatchers.IO) {
                    try {
                        VisioManager.client.setDisplayName(displayName.ifBlank { null })
                        VisioManager.client.setLanguage(language)
                        VisioManager.client.setMicEnabledOnJoin(micOnJoin)
                        VisioManager.client.setCameraEnabledOnJoin(cameraOnJoin)
                    } catch (_: Exception) {}
                }
                onBack()
            },
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = VisioColors.Primary500,
                contentColor = VisioColors.White
            ),
            shape = RoundedCornerShape(12.dp)
        ) {
            Text("Save", modifier = Modifier.padding(vertical = 4.dp))
        }
    }
}

@Composable
private fun SectionHeader(title: String) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleMedium,
        color = VisioColors.White
    )
}

@Composable
private fun SettingsToggle(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(VisioColors.PrimaryDark100, RoundedCornerShape(12.dp))
            .padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = VisioColors.White
        )
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = VisioColors.White,
                checkedTrackColor = VisioColors.Primary500,
                uncheckedThumbColor = VisioColors.Greyscale400,
                uncheckedTrackColor = VisioColors.PrimaryDark300
            )
        )
    }
}

@Composable
private fun LanguageOption(
    label: String,
    value: String,
    selected: String,
    onSelect: (String) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .selectable(
                selected = value == selected,
                onClick = { onSelect(value) },
                role = Role.RadioButton
            )
            .background(VisioColors.PrimaryDark100, RoundedCornerShape(12.dp))
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        RadioButton(
            selected = value == selected,
            onClick = null,
            colors = RadioButtonDefaults.colors(
                selectedColor = VisioColors.Primary500,
                unselectedColor = VisioColors.Greyscale400
            )
        )
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = VisioColors.White
        )
    }
}
