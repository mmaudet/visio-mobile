package io.visio.mobile.ui

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.visio.CalendarRefreshInterval

private const val TAG = "SettingsScreen"

@Suppress("kotlin:S3776", "kotlin:S6615")
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onBack: () -> Unit) {
    var displayName by remember { mutableStateOf("") }
    var language by remember { mutableStateOf(Strings.detectSystemLang()) }
    var theme by remember { mutableStateOf("light") }
    var micOnJoin by remember { mutableStateOf(true) }
    var cameraOnJoin by remember { mutableStateOf(false) }
    var adaptiveModeEnabled by remember { mutableStateOf(false) }
    var meetInstances by remember { mutableStateOf(listOf("meet.numerique.gouv.fr")) }
    var newInstance by remember { mutableStateOf("") }
    var calendarUrl by remember { mutableStateOf("") }
    var calendarRefreshInterval by remember { mutableStateOf(CalendarRefreshInterval.MINUTES15) }
    val coroutineScope = rememberCoroutineScope()
    val context = androidx.compose.ui.platform.LocalContext.current

    // Use VisioManager.currentLang for live i18n (updates instantly when language radio changes)
    val lang = VisioManager.currentLang
    val isDark = VisioManager.currentTheme == "dark"

    // Load current settings
    LaunchedEffect(Unit) {
        try {
            val settings = VisioManager.client.getSettings()
            displayName = settings.displayName ?: ""
            language = settings.language ?: Strings.detectSystemLang()
            theme = settings.theme ?: "light"
            micOnJoin = settings.micEnabledOnJoin
            cameraOnJoin = settings.cameraEnabledOnJoin
            adaptiveModeEnabled = VisioManager.client.isAdaptiveModeEnabled()
            meetInstances = VisioManager.client.getMeetInstances()
            calendarUrl = VisioManager.client.getCalendarUrl() ?: ""
            calendarRefreshInterval = VisioManager.client.getCalendarRefreshInterval()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to load settings", e)
        }
    }

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .statusBarsPadding()
                .navigationBarsPadding()
                .imePadding(),
    ) {
        TopAppBar(
            title = {
                Text(Strings.t("settings", lang), color = MaterialTheme.colorScheme.onSurface)
            },
            navigationIcon = {
                IconButton(onClick = onBack, modifier = Modifier.testTag("settings_back_button")) {
                    Icon(
                        painter = painterResource(R.drawable.ri_arrow_left_s_line),
                        contentDescription = "Back",
                        tint = MaterialTheme.colorScheme.onSurface,
                    )
                }
            },
            colors =
                TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                ),
        )

        Column(
            modifier =
                Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            // Profile section
            SectionHeader(Strings.t("settings.profile", lang), isDark)
            Text(
                text = Strings.t("settings.displayName", lang),
                style = MaterialTheme.typography.bodyMedium,
                color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
            )
            TextField(
                value = displayName,
                onValueChange = { displayName = it },
                placeholder = {
                    Text(
                        Strings.t("home.displayName.placeholder", lang),
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    )
                },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().testTag("settings_display_name_input"),
                colors =
                    TextFieldDefaults.colors(
                        focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                        unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                        cursorColor = VisioColors.Primary500,
                        focusedTextColor = MaterialTheme.colorScheme.onSurface,
                        unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                    ),
                shape = RoundedCornerShape(12.dp),
            )

            // Join meeting section
            SectionHeader(Strings.t("settings.joinMeeting", lang), isDark)
            SettingsToggle(
                label = Strings.t("settings.micOnJoin", lang),
                checked = micOnJoin,
                onCheckedChange = { micOnJoin = it },
                isDark = isDark,
            )
            SettingsToggle(
                label = Strings.t("settings.camOnJoin", lang),
                checked = cameraOnJoin,
                onCheckedChange = { cameraOnJoin = it },
                isDark = isDark,
            )
            SettingsToggle(
                label = Strings.t("settings.adaptiveMode", lang),
                checked = adaptiveModeEnabled,
                onCheckedChange = { adaptiveModeEnabled = it },
                isDark = isDark,
            )

            // Calendar section
            SectionHeader(Strings.t("settings.calendar", lang), isDark)
            OutlinedTextField(
                value = calendarUrl,
                onValueChange = { calendarUrl = it },
                label = {
                    Text(
                        Strings.t("settings.calendarUrl", lang),
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    )
                },
                placeholder = {
                    Text(
                        "https://example.com/calendar.ics",
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    )
                },
                singleLine = true,
                keyboardOptions =
                    KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        autoCorrectEnabled = false,
                        capitalization = KeyboardCapitalization.None,
                    ),
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp),
            )
            CalendarIntervalDropdown(
                selected = calendarRefreshInterval,
                isDark = isDark,
                onSelect = { interval ->
                    calendarRefreshInterval = interval
                    coroutineScope.launch(Dispatchers.IO) {
                        try {
                            VisioManager.client.setCalendarRefreshInterval(interval)
                        } catch (e: Exception) {
                            Log.e(TAG, "Failed to save calendar refresh interval", e)
                        }
                    }
                },
                lang = lang,
            )
            if (calendarUrl.isNotBlank()) {
                Button(
                    onClick = {
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                VisioManager.client.setCalendarUrl(null)
                            } catch (e: Exception) {
                                Log.e(TAG, "Failed to delete calendar URL", e)
                            }
                        }
                        calendarUrl = ""
                    },
                    modifier = Modifier.fillMaxWidth(),
                    colors =
                        ButtonDefaults.buttonColors(
                            containerColor = Color(0xFFE1000F),
                            contentColor = VisioColors.White,
                        ),
                    shape = RoundedCornerShape(12.dp),
                ) {
                    Text(
                        Strings.t("settings.calendarRemove", lang),
                        modifier = Modifier.padding(vertical = 4.dp),
                    )
                }
            }

            // Theme section
            SectionHeader(Strings.t("settings.theme", lang), isDark)
            ThemeOption(Strings.t("settings.theme.light", lang), "light", theme, isDark) {
                theme = it
                VisioManager.setTheme(it)
            }
            ThemeOption(Strings.t("settings.theme.dark", lang), "dark", theme, isDark) {
                theme = it
                VisioManager.setTheme(it)
            }

            // Language section
            SectionHeader(Strings.t("settings.language", lang), isDark)
            LanguageDropdown(
                selected = language,
                isDark = isDark,
                onSelect = {
                    language = it
                    VisioManager.setLanguage(it)
                },
            )

            // Meet instances section
            SectionHeader(Strings.t("settings.meetInstances", lang), isDark)
            Column(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .background(
                            if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                            RoundedCornerShape(12.dp),
                        )
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                meetInstances.forEachIndexed { index, instance ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = instance,
                            style = MaterialTheme.typography.bodyLarge,
                            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(
                            onClick = {
                                meetInstances = meetInstances.toMutableList().also { it.removeAt(index) }
                            },
                            modifier = Modifier.size(32.dp),
                        ) {
                            Icon(
                                painter = painterResource(R.drawable.ri_close_line),
                                contentDescription = "Remove",
                                tint = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                            )
                        }
                    }
                }
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextField(
                    value = newInstance,
                    onValueChange = { newInstance = it },
                    placeholder = {
                        Text(
                            Strings.t("settings.addInstance", lang),
                            color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                        )
                    },
                    singleLine = true,
                    keyboardOptions =
                        KeyboardOptions(
                            keyboardType = KeyboardType.Uri,
                            autoCorrectEnabled = false,
                            capitalization = KeyboardCapitalization.None,
                        ),
                    modifier = Modifier.weight(1f),
                    colors =
                        TextFieldDefaults.colors(
                            focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                            unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                            cursorColor = VisioColors.Primary500,
                            focusedTextColor = MaterialTheme.colorScheme.onSurface,
                            unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                            focusedIndicatorColor = Color.Transparent,
                            unfocusedIndicatorColor = Color.Transparent,
                        ),
                    shape = RoundedCornerShape(12.dp),
                )
                IconButton(
                    onClick = {
                        val trimmed = newInstance.trim()
                        if (trimmed.isNotEmpty() && trimmed !in meetInstances) {
                            meetInstances = meetInstances + trimmed
                            newInstance = ""
                        }
                    },
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ri_add_line),
                        contentDescription = "Add",
                        tint = VisioColors.Primary500,
                    )
                }
            }
        }

        SettingsSaveButton(
            displayName = displayName,
            language = language,
            micOnJoin = micOnJoin,
            cameraOnJoin = cameraOnJoin,
            adaptiveModeEnabled = adaptiveModeEnabled,
            meetInstances = meetInstances,
            newInstance = newInstance,
            calendarUrl = calendarUrl,
            lang = lang,
            context = context,
            coroutineScope = coroutineScope,
            onBack = onBack,
        )
    }
}

@Suppress("kotlin:S107", "kotlin:S3776")
@Composable
private fun SettingsSaveButton(
    displayName: String,
    language: String,
    micOnJoin: Boolean,
    cameraOnJoin: Boolean,
    adaptiveModeEnabled: Boolean,
    meetInstances: List<String>,
    newInstance: String,
    calendarUrl: String,
    lang: String,
    context: android.content.Context,
    coroutineScope: kotlinx.coroutines.CoroutineScope,
    onBack: () -> Unit,
) {
    Button(
        onClick = {
            val trimmed = newInstance.trim()
            val instancesToSave =
                if (trimmed.isNotEmpty() && trimmed !in meetInstances) {
                    meetInstances + trimmed
                } else {
                    meetInstances
                }
            coroutineScope.launch(Dispatchers.IO) {
                saveSettings(
                    displayName,
                    language,
                    micOnJoin,
                    cameraOnJoin,
                    adaptiveModeEnabled,
                    instancesToSave,
                    calendarUrl,
                )
            }
            VisioManager.updateDisplayName(displayName)
            android.widget.Toast.makeText(
                context,
                Strings.t("settings.saved", lang),
                android.widget.Toast.LENGTH_SHORT,
            ).show()
            onBack()
        },
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(16.dp),
        colors =
            ButtonDefaults.buttonColors(
                containerColor = VisioColors.Primary500,
                contentColor = VisioColors.White,
            ),
        shape = RoundedCornerShape(12.dp),
    ) {
        Text(Strings.t("settings.save", lang), modifier = Modifier.padding(vertical = 4.dp))
    }
}

@Suppress("kotlin:S107", "kotlin:S3776")
private fun saveSettings(
    displayName: String,
    language: String,
    micOnJoin: Boolean,
    cameraOnJoin: Boolean,
    adaptiveModeEnabled: Boolean,
    instancesToSave: List<String>,
    calendarUrl: String,
) {
    try {
        VisioManager.client.setDisplayName(displayName.ifBlank { null })
        VisioManager.client.setLanguage(language)
        VisioManager.client.setMicEnabledOnJoin(micOnJoin)
        VisioManager.client.setCameraEnabledOnJoin(cameraOnJoin)
        val wasEnabled = VisioManager.client.isAdaptiveModeEnabled()
        VisioManager.client.setAdaptiveModeEnabled(adaptiveModeEnabled)
        syncContextDetection(wasEnabled, adaptiveModeEnabled)
        VisioManager.client.setMeetInstances(instancesToSave)
        val calUrl = calendarUrl.trim().ifBlank { null }
        VisioManager.client.setCalendarUrl(calUrl)
        if (calUrl != null) {
            VisioManager.refreshCalendarNow()
        }
    } catch (e: Exception) {
        Log.e(TAG, "Failed to save settings", e)
    }
}

private fun syncContextDetection(
    wasEnabled: Boolean,
    isEnabled: Boolean,
) {
    if (wasEnabled && !isEnabled) {
        VisioManager.stopContextDetection()
    } else if (!wasEnabled && isEnabled) {
        VisioManager.startContextDetection()
    }
}

@Composable
private fun SectionHeader(
    title: String,
    isDark: Boolean,
) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleMedium,
        color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
    )
}

@Composable
private fun SettingsToggle(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    isDark: Boolean,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(
                    if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    RoundedCornerShape(12.dp),
                )
                .padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
            modifier = Modifier.weight(1f),
        )
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors =
                SwitchDefaults.colors(
                    checkedThumbColor = VisioColors.White,
                    checkedTrackColor = VisioColors.Primary500,
                    uncheckedThumbColor = VisioColors.Greyscale400,
                    uncheckedTrackColor = if (isDark) VisioColors.PrimaryDark300 else VisioColors.LightBorder,
                ),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LanguageDropdown(
    selected: String,
    isDark: Boolean,
    onSelect: (String) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }

    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { expanded = it },
    ) {
        TextField(
            value = Strings.t("lang.$selected", selected),
            onValueChange = {},
            readOnly = true,
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier =
                Modifier
                    .menuAnchor(MenuAnchorType.PrimaryNotEditable)
                    .fillMaxWidth(),
            colors =
                TextFieldDefaults.colors(
                    focusedContainerColor = if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    unfocusedContainerColor = if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    focusedTextColor = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    unfocusedTextColor = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    focusedTrailingIconColor = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    unfocusedTrailingIconColor = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                ),
            shape = RoundedCornerShape(12.dp),
        )
        ExposedDropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
            containerColor = if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
        ) {
            Strings.supportedLangs.forEach { code ->
                DropdownMenuItem(
                    text = {
                        Text(
                            Strings.t("lang.$code", code),
                            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                        )
                    },
                    onClick = {
                        onSelect(code)
                        expanded = false
                    },
                    modifier = Modifier.testTag("settings_language_$code"),
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CalendarIntervalDropdown(
    selected: CalendarRefreshInterval,
    isDark: Boolean,
    lang: String,
    onSelect: (CalendarRefreshInterval) -> Unit,
) {
    val intervalLabels =
        listOf(
            CalendarRefreshInterval.MINUTES5 to Strings.t("settings.calendarRefresh.5min", lang),
            CalendarRefreshInterval.MINUTES15 to Strings.t("settings.calendarRefresh.15min", lang),
            CalendarRefreshInterval.HOUR1 to Strings.t("settings.calendarRefresh.1h", lang),
            CalendarRefreshInterval.HOURS4 to Strings.t("settings.calendarRefresh.4h", lang),
            CalendarRefreshInterval.MANUAL to Strings.t("settings.calendarRefresh.manual", lang),
        )
    val selectedLabel = intervalLabels.firstOrNull { it.first == selected }?.second ?: selected.name
    var expanded by remember { mutableStateOf(false) }

    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { expanded = it },
    ) {
        TextField(
            value = selectedLabel,
            onValueChange = {},
            readOnly = true,
            label = {
                Text(
                    Strings.t("settings.calendarRefresh", lang),
                    color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                )
            },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier =
                Modifier
                    .menuAnchor(MenuAnchorType.PrimaryNotEditable)
                    .fillMaxWidth(),
            colors =
                TextFieldDefaults.colors(
                    focusedContainerColor = if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    unfocusedContainerColor = if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    focusedTextColor = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    unfocusedTextColor = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    focusedTrailingIconColor = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                    unfocusedTrailingIconColor = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                ),
            shape = RoundedCornerShape(12.dp),
        )
        ExposedDropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
            containerColor = if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
        ) {
            intervalLabels.forEach { pair ->
                DropdownMenuItem(
                    text = {
                        Text(
                            pair.second,
                            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                        )
                    },
                    onClick = {
                        onSelect(pair.first)
                        expanded = false
                    },
                )
            }
        }
    }
}

@Composable
private fun ThemeOption(
    label: String,
    value: String,
    selected: String,
    isDark: Boolean,
    onSelect: (String) -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .selectable(
                    selected = value == selected,
                    onClick = { onSelect(value) },
                    role = Role.RadioButton,
                )
                .background(
                    if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    RoundedCornerShape(12.dp),
                )
                .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        RadioButton(
            selected = value == selected,
            onClick = null,
            colors =
                RadioButtonDefaults.colors(
                    selectedColor = VisioColors.Primary500,
                    unselectedColor = VisioColors.Greyscale400,
                ),
        )
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
        )
    }
}
