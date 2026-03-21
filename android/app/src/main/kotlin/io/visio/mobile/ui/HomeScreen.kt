package io.visio.mobile.ui

import android.content.Intent
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.Smartphone
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.TabRowDefaults
import androidx.compose.material3.TabRowDefaults.tabIndicatorOffset
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.visio.mobile.R
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.i18n.Strings
import io.visio.mobile.ui.theme.VisioColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.visio.RoomValidationResult
import uniffi.visio.UserSearchResult

private const val TAG = "HomeScreen"

@Composable
fun HomeScreen(
    onJoin: (roomUrl: String, username: String) -> Unit,
    onSettings: () -> Unit,
) {
    val context = LocalContext.current
    var roomUrl by remember { mutableStateOf("") }
    var resolvedRoomUrl by remember { mutableStateOf("") }
    var username by remember { mutableStateOf("") }
    var roomHistory by remember { mutableStateOf(listOf<String>()) }
    val lang = VisioManager.currentLang
    val isDark = VisioManager.currentTheme == "dark"
    var roomStatus by remember { mutableStateOf("idle") }
    val slugRegex = remember { Regex("^[a-z]{3}-[a-z]{4}-[a-z]{3}$") }
    var meetInstances by remember { mutableStateOf(listOf<String>()) }
    var showServerPicker by remember { mutableStateOf(false) }
    var showCreateRoom by remember { mutableStateOf(false) }
    var customServer by remember { mutableStateOf("") }
    var historyJoining by remember { mutableStateOf<String?>(null) }
    val coroutineScope = rememberCoroutineScope()
    // Calendar / Meetings tab state
    var selectedTab by remember { mutableIntStateOf(0) }
    val upcomingMeetings by VisioManager.upcomingMeetings.collectAsState()
    val calendarLoading by VisioManager.calendarLoading.collectAsState()
    var hasCalendarUrl by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        try {
            roomHistory = VisioManager.client.getRoomHistory()
            hasCalendarUrl = VisioManager.client.getCalendarUrl() != null
        } catch (e: Exception) {
            Log.e(TAG, "Failed to load room history", e)
        }
    }

    // Reload calendar URL state when returning from Settings
    androidx.lifecycle.compose.LifecycleResumeEffect("calendar_url") {
        try {
            hasCalendarUrl = VisioManager.client.getCalendarUrl() != null
        } catch (_: Exception) {
        }
        onPauseOrDispose {}
    }

    // Refresh calendar when switching to Réunions tab
    LaunchedEffect(selectedTab) {
        if (selectedTab == 1) {
            VisioManager.refreshCalendarNow()
        }
    }

    LaunchedEffect(VisioManager.pendingDeepLink) {
        val link = VisioManager.pendingDeepLink
        if (link != null) {
            roomUrl = link
            VisioManager.pendingDeepLink = null
        }
    }

    // Reload meet instances when the screen becomes visible (e.g. returning
    // from Settings where the user may have added/removed instances).
    androidx.lifecycle.compose.LifecycleResumeEffect(Unit) {
        try {
            meetInstances = VisioManager.client.getMeetInstances()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to load meet instances", e)
        }
        onPauseOrDispose {}
    }

    LaunchedEffect(roomUrl) {
        val trimmed = roomUrl.trim()
        val isSlug = slugRegex.matches(trimmed)
        val candidate =
            if (isSlug) {
                trimmed
            } else {
                val stripped = trimmed.trimEnd('/')
                if ('/' in stripped) stripped.substringAfterLast('/') else stripped
            }
        if (!slugRegex.matches(candidate)) {
            roomStatus = "idle"
            resolvedRoomUrl = trimmed
            return@LaunchedEffect
        }
        roomStatus = "checking"
        delay(500)
        // If input is a slug, try each configured server; otherwise validate the full URL
        val urlsToTry =
            if (isSlug && meetInstances.isNotEmpty()) {
                meetInstances.map { server -> "https://$server/$trimmed" }
            } else {
                listOf(trimmed)
            }
        try {
            var foundValid = false
            for (url in urlsToTry) {
                val result =
                    withContext(Dispatchers.IO) {
                        VisioManager.client.validateRoom(url, username.trim().ifEmpty { null })
                    }
                when (result) {
                    is RoomValidationResult.Valid -> {
                        roomStatus = "valid"
                        resolvedRoomUrl = url
                        foundValid = true
                        break
                    }
                    else -> continue
                }
            }
            if (!foundValid) {
                roomStatus = "not_found"
                resolvedRoomUrl = urlsToTry.first()
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to validate room URL", e)
            roomStatus = "error"
        }
    }

    // Pre-fill display name from VisioManager observable state
    LaunchedEffect(VisioManager.displayName) {
        val name = VisioManager.displayName
        if (name.isNotBlank() && username.isEmpty()) {
            username = name
        }
    }

    // Check if any meeting is imminent (< 15 min) for badge indicator
    val nowSeconds = System.currentTimeMillis() / 1000L
    val hasImminentMeeting =
        upcomingMeetings.any { meeting ->
            val minutesUntil = (meeting.startTime - nowSeconds) / 60
            minutesUntil in 0..14
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
        // Header row (always visible)
        Column(modifier = Modifier.padding(horizontal = 32.dp).padding(top = 32.dp)) {
            // Title row with settings gear
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Spacer(modifier = Modifier.size(48.dp)) // balance the gear icon
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    VisioLogo(size = 120.dp)
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = Strings.t("app.title", lang),
                        style = MaterialTheme.typography.headlineLarge,
                        color = MaterialTheme.colorScheme.onBackground,
                        fontWeight = FontWeight.Bold,
                    )
                }
                IconButton(
                    onClick = onSettings,
                    modifier = Modifier.size(48.dp).testTag("home_settings_button"),
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ri_settings_3_line),
                        contentDescription = Strings.t("settings", lang),
                        tint = if (isDark) VisioColors.White else VisioColors.Greyscale400,
                        modifier = Modifier.size(24.dp),
                    )
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            Text(
                text = Strings.t("home.subtitle", lang),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onBackground.copy(alpha = 0.7f),
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Connect / Logout section
            if (VisioManager.isAuthenticated) {
                AuthenticatedCard(
                    displayName = VisioManager.authenticatedDisplayName,
                    email = VisioManager.authenticatedEmail,
                    isDark = isDark,
                    lang = lang,
                    onLogout = { VisioManager.logout() },
                )
            } else {
                Button(
                    onClick = {
                        if (meetInstances.size <= 1) {
                            val meetInstance = meetInstances.firstOrNull() ?: return@Button
                            VisioManager.authManager.launchOidcFlow(context, meetInstance)
                        } else {
                            customServer = ""
                            showServerPicker = true
                        }
                    },
                    modifier = Modifier.fillMaxWidth().testTag("home_connect_button"),
                    colors =
                        ButtonDefaults.buttonColors(
                            containerColor = VisioColors.Primary500,
                            contentColor = VisioColors.White,
                        ),
                    shape = RoundedCornerShape(12.dp),
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ri_account_circle_line),
                        contentDescription = null,
                        modifier = Modifier.size(20.dp),
                    )
                    Text(
                        Strings.t("home.connect", lang),
                        fontSize = 16.sp,
                        modifier = Modifier.padding(start = 8.dp, top = 4.dp, bottom = 4.dp),
                    )
                }
            }

            if (showServerPicker) {
                ServerPickerDialog(
                    instances = meetInstances,
                    customServer = customServer,
                    onCustomServerChange = { customServer = it },
                    lang = lang,
                    onSelect = { instance ->
                        showServerPicker = false
                        VisioManager.authManager.launchOidcFlow(context, instance)
                    },
                    onDismiss = { showServerPicker = false },
                )
            }
        } // end header Column

        // Tab strip (Rejoindre / Réunions)
        run {
            TabRow(
                selectedTabIndex = selectedTab,
                containerColor = MaterialTheme.colorScheme.surface,
                contentColor = MaterialTheme.colorScheme.onSurface,
                indicator = { tabPositions ->
                    TabRowDefaults.SecondaryIndicator(
                        modifier = Modifier.tabIndicatorOffset(tabPositions[selectedTab]),
                        color = VisioColors.Primary500,
                    )
                },
            ) {
                Tab(
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    text = {
                        Text(
                            if (lang == "fr") "Rejoindre" else "Join",
                            color =
                                if (selectedTab == 0) {
                                    VisioColors.Primary500
                                } else {
                                    if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary
                                },
                        )
                    },
                )
                Tab(
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    text = {
                        BadgedBox(
                            badge = {
                                if (upcomingMeetings.isNotEmpty() || hasImminentMeeting) {
                                    Badge(
                                        containerColor =
                                            if (hasImminentMeeting) Color(0xFFE1000F) else VisioColors.Primary500,
                                    ) {
                                        if (upcomingMeetings.isNotEmpty()) {
                                            Text(
                                                upcomingMeetings.size.toString(),
                                                color = VisioColors.White,
                                            )
                                        }
                                    }
                                }
                            },
                        ) {
                            Text(
                                if (lang == "fr") "Réunions planifiées" else "Scheduled meetings",
                                color =
                                    if (selectedTab == 1) {
                                        VisioColors.Primary500
                                    } else {
                                        if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary
                                    },
                            )
                        }
                    },
                )
            }
        }

        // Tab content
        Box(modifier = Modifier.weight(1f)) {
            if (selectedTab == 0) {
                // Join tab content (scrollable)
                Column(
                    modifier =
                        Modifier
                            .fillMaxSize()
                            .verticalScroll(rememberScrollState())
                            .padding(horizontal = 32.dp)
                            .padding(bottom = 32.dp),
                    verticalArrangement = Arrangement.Top,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Spacer(modifier = Modifier.height(16.dp))

                    Text(
                        text = Strings.t("home.meetUrl", lang),
                        style = MaterialTheme.typography.bodySmall,
                        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                        modifier = Modifier.fillMaxWidth().padding(bottom = 4.dp),
                    )
                    TextField(
                        value = roomUrl,
                        onValueChange = { roomUrl = it },
                        placeholder = {
                            Text(
                                "abc-defg-hij",
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
                        modifier = Modifier.fillMaxWidth().testTag("home_room_url_input"),
                        colors =
                            TextFieldDefaults.colors(
                                focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                                unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                                cursorColor = VisioColors.Primary500,
                                focusedTextColor = MaterialTheme.colorScheme.onSurface,
                                unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                                focusedLabelColor = VisioColors.Primary500,
                                unfocusedLabelColor = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                                focusedIndicatorColor = Color.Transparent,
                                unfocusedIndicatorColor = Color.Transparent,
                            ),
                        shape = RoundedCornerShape(12.dp),
                    )

                    when (roomStatus) {
                        "checking" ->
                            Text(
                                Strings.t("home.room.checking", lang),
                                style = MaterialTheme.typography.bodySmall,
                                color = VisioColors.Greyscale400,
                                modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                                textAlign = androidx.compose.ui.text.style.TextAlign.End,
                            )
                        "valid" ->
                            Text(
                                Strings.t("home.room.valid", lang),
                                style = MaterialTheme.typography.bodySmall,
                                color = Color(0xFF18753C),
                                modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                                textAlign = androidx.compose.ui.text.style.TextAlign.End,
                            )
                        "not_found" ->
                            Text(
                                Strings.t("home.room.notFound", lang),
                                style = MaterialTheme.typography.bodySmall,
                                color = Color(0xFFE1000F),
                                modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                                textAlign = androidx.compose.ui.text.style.TextAlign.End,
                            )
                    }

                    Spacer(modifier = Modifier.height(16.dp))

                    TextField(
                        value = username,
                        onValueChange = { username = it },
                        label = {
                            Text(
                                Strings.t("home.displayName", lang),
                                color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                            )
                        },
                        placeholder = {
                            Text(
                                Strings.t("home.displayName.placeholder", lang),
                                color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                            )
                        },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().testTag("home_display_name_input"),
                        colors =
                            TextFieldDefaults.colors(
                                focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                                unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
                                cursorColor = VisioColors.Primary500,
                                focusedTextColor = MaterialTheme.colorScheme.onSurface,
                                unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                                focusedLabelColor = VisioColors.Primary500,
                                unfocusedLabelColor = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                                focusedIndicatorColor = Color.Transparent,
                                unfocusedIndicatorColor = Color.Transparent,
                            ),
                        shape = RoundedCornerShape(12.dp),
                    )

                    Spacer(modifier = Modifier.height(24.dp))

                    Button(
                        onClick = { onJoin(resolvedRoomUrl, username.trim()) },
                        enabled = roomStatus == "valid",
                        modifier = Modifier.fillMaxWidth().testTag("home_join_button"),
                        colors =
                            ButtonDefaults.buttonColors(
                                containerColor = VisioColors.Primary500,
                                contentColor = VisioColors.White,
                                disabledContainerColor = VisioColors.PrimaryDark300,
                                disabledContentColor = VisioColors.Greyscale400,
                            ),
                        shape = RoundedCornerShape(12.dp),
                    ) {
                        Text(
                            Strings.t("home.join", lang),
                            fontSize = 16.sp,
                            modifier = Modifier.padding(vertical = 4.dp),
                        )
                    }

                    if (VisioManager.isAuthenticated) {
                        Spacer(modifier = Modifier.height(12.dp))
                        OutlinedButton(
                            onClick = { showCreateRoom = true },
                            modifier = Modifier.fillMaxWidth().testTag("home_create_room_button"),
                            shape = RoundedCornerShape(12.dp),
                        ) {
                            Text(
                                Strings.t("home.createRoom", lang),
                                fontSize = 16.sp,
                                modifier = Modifier.padding(vertical = 4.dp),
                            )
                        }
                    }

                    // Room history
                    if (roomHistory.isNotEmpty()) {
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = Strings.t("home.recentRooms", lang),
                            style = MaterialTheme.typography.titleSmall,
                            color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        roomHistory.forEachIndexed { index, url ->
                            val slug = if ('/' in url) url.substringAfterLast('/') else url
                            val host =
                                try {
                                    java.net.URI(url).host ?: ""
                                } catch (_: Exception) {
                                    ""
                                }
                            Row(
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .testTag("home_room_history_item_$index")
                                        .clickable(enabled = historyJoining == null) {
                                            roomUrl = url
                                            historyJoining = url
                                            coroutineScope.launch {
                                                try {
                                                    val uname = username.trim().ifEmpty { null }
                                                    val result =
                                                        withContext(Dispatchers.IO) {
                                                            VisioManager.client.validateRoom(url, uname)
                                                        }
                                                    if (result is RoomValidationResult.Valid) {
                                                        historyJoining = null
                                                        onJoin(url, username.trim())
                                                    } else {
                                                        // Validation failed — fall back to filling the URL field
                                                        historyJoining = null
                                                    }
                                                } catch (e: Exception) {
                                                    Log.e(TAG, "Failed to join from history", e)
                                                    historyJoining = null
                                                }
                                            }
                                        }
                                        .background(
                                            VisioColors.Primary500.copy(alpha = if (isDark) 0.12f else 0.08f),
                                            RoundedCornerShape(8.dp),
                                        )
                                        .padding(horizontal = 12.dp, vertical = 10.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                if (historyJoining == url) {
                                    androidx.compose.material3.CircularProgressIndicator(
                                        modifier = Modifier.size(18.dp),
                                        strokeWidth = 2.dp,
                                        color = VisioColors.Primary500,
                                    )
                                } else {
                                    Icon(
                                        imageVector = Icons.Default.Public,
                                        contentDescription = null,
                                        tint = VisioColors.Primary500,
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = slug,
                                        style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = FontWeight.Medium,
                                        color = MaterialTheme.colorScheme.onSurface,
                                    )
                                    if (host.isNotEmpty()) {
                                        Text(
                                            text = host,
                                            style = MaterialTheme.typography.bodySmall,
                                            color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                                        )
                                    }
                                }
                            }
                            Spacer(modifier = Modifier.height(6.dp))
                        }
                    } // end if roomHistory
                } // end join tab scrollable column
            } // end if tab 0

            // Réunions tab
            if (VisioManager.isAuthenticated && selectedTab == 1) {
                MeetingsTab(
                    meetings = upcomingMeetings,
                    hasCalendarUrl = hasCalendarUrl,
                    isLoading = calendarLoading,
                    isDark = isDark,
                    lang = lang,
                    onSettings = onSettings,
                    onJoinMeeting = { meetingRoomUrl, _ ->
                        roomUrl = meetingRoomUrl
                        selectedTab = 0
                    },
                )
            }
        } // end Box
    } // end outer Column

    if (showCreateRoom) {
        CreateRoomDialog(
            meetInstance = VisioManager.authenticatedMeetInstance,
            lang = lang,
            onCreated = { roomUrl ->
                showCreateRoom = false
                onJoin(roomUrl, username)
            },
            onDismiss = { showCreateRoom = false },
        )
    }
}

@Composable
private fun AuthenticatedCard(
    displayName: String,
    email: String,
    isDark: Boolean,
    lang: String,
    onLogout: () -> Unit,
) {
    val initials =
        displayName
            .split(" ")
            .filter { it.isNotEmpty() }
            .take(2)
            .joinToString("") { it.first().uppercase() }
            .ifEmpty { email.firstOrNull()?.uppercase()?.toString() ?: "?" }

    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .background(
                    color = if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                    shape = RoundedCornerShape(16.dp),
                )
                .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // Avatar circle with initials
        Box(
            modifier =
                Modifier
                    .size(44.dp)
                    .background(
                        color = VisioColors.Primary500,
                        shape = RoundedCornerShape(22.dp),
                    ),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = initials,
                color = VisioColors.White,
                fontWeight = FontWeight.Bold,
                fontSize = 16.sp,
            )
        }
        // Name and email
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = displayName.ifEmpty { email },
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            )
            if (email.isNotEmpty() && displayName.isNotEmpty()) {
                Text(
                    text = email,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                    maxLines = 1,
                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                )
            }
        }
        // Logout button
        IconButton(
            onClick = onLogout,
            modifier = Modifier.size(36.dp),
        ) {
            Icon(
                painter = painterResource(R.drawable.ri_logout_box_r_line),
                contentDescription = Strings.t("home.logout", lang),
                tint = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
                modifier = Modifier.size(20.dp),
            )
        }
    }
}

@Composable
private fun CreateRoomDialog(
    meetInstance: String,
    lang: String,
    onCreated: (roomUrl: String) -> Unit,
    onDismiss: () -> Unit,
) {
    if (meetInstance.isEmpty()) return
    var accessLevel by remember { mutableStateOf("public") }
    var creating by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var createdUrl by remember { mutableStateOf<String?>(null) }
    var searchQuery by remember { mutableStateOf("") }
    var searchResults by remember { mutableStateOf<List<UserSearchResult>>(emptyList()) }
    var invitedUsers by remember { mutableStateOf<List<UserSearchResult>>(emptyList()) }
    var createdRoomId by remember { mutableStateOf<String?>(null) }
    val coroutineScope = rememberCoroutineScope()
    val context = LocalContext.current
    val clipboardManager = LocalClipboardManager.current

    LaunchedEffect(searchQuery) {
        if (searchQuery.length < 3) {
            searchResults = emptyList()
            return@LaunchedEffect
        }
        delay(300)
        try {
            val results = VisioManager.client.searchUsers(searchQuery)
            searchResults =
                results.filter { user ->
                    invitedUsers.none { it.id == user.id }
                }
        } catch (_: Exception) {
            searchResults = emptyList()
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(Strings.t("home.createRoom", lang)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (createdUrl == null) {
                    Text(
                        text = Strings.t("home.createRoom.access", lang),
                        style = MaterialTheme.typography.labelMedium,
                    )

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(
                            selected = accessLevel == "public",
                            onClick = { accessLevel = "public" },
                        )
                        Column(modifier = Modifier.padding(start = 4.dp)) {
                            Text(Strings.t("home.createRoom.public", lang), style = MaterialTheme.typography.bodyMedium)
                            Text(Strings.t("home.createRoom.publicDesc", lang), style = MaterialTheme.typography.bodySmall)
                        }
                    }

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(
                            selected = accessLevel == "trusted",
                            onClick = { accessLevel = "trusted" },
                        )
                        Column(modifier = Modifier.padding(start = 4.dp)) {
                            Text(Strings.t("home.createRoom.trusted", lang), style = MaterialTheme.typography.bodyMedium)
                            Text(Strings.t("home.createRoom.trustedDesc", lang), style = MaterialTheme.typography.bodySmall)
                        }
                    }

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(
                            selected = accessLevel == "restricted",
                            onClick = { accessLevel = "restricted" },
                        )
                        Column(modifier = Modifier.padding(start = 4.dp)) {
                            Text(Strings.t("home.createRoom.restricted", lang), style = MaterialTheme.typography.bodyMedium)
                            Text(Strings.t("home.createRoom.restrictedDesc", lang), style = MaterialTheme.typography.bodySmall)
                        }
                    }

                    if (accessLevel == "restricted") {
                        Text(
                            text = Strings.t("restricted.invite", lang),
                            style = MaterialTheme.typography.labelMedium,
                        )
                        OutlinedTextField(
                            value = searchQuery,
                            onValueChange = { searchQuery = it },
                            placeholder = { Text(Strings.t("restricted.searchUsers", lang)) },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            textStyle = MaterialTheme.typography.bodySmall,
                        )
                        // Search results dropdown
                        searchResults.forEach { user ->
                            Row(
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .clickable {
                                            invitedUsers = invitedUsers + user
                                            searchQuery = ""
                                            searchResults = emptyList()
                                        }
                                        .padding(vertical = 6.dp, horizontal = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        user.fullName ?: user.email,
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                    Text(
                                        user.email,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                        // Invited user chips
                        if (invitedUsers.isNotEmpty()) {
                            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                invitedUsers.forEach { user ->
                                    Row(
                                        modifier = Modifier.fillMaxWidth(),
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        Text(
                                            user.fullName ?: user.email,
                                            style = MaterialTheme.typography.bodySmall,
                                            modifier = Modifier.weight(1f),
                                        )
                                        IconButton(
                                            onClick = { invitedUsers = invitedUsers.filter { it.id != user.id } },
                                            modifier = Modifier.size(24.dp),
                                        ) {
                                            Icon(
                                                Icons.Default.Close,
                                                contentDescription = Strings.t("restricted.remove", lang),
                                                modifier = Modifier.size(16.dp),
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if (error != null) {
                        Text(
                            text = error!!,
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                } else {
                    val deepLink = "visio://${createdUrl!!.removePrefix("https://")}"

                    Text(
                        text = Strings.t("settings.incall.roomInfo", lang),
                        style = MaterialTheme.typography.labelMedium,
                    )

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Default.Public, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(
                            Strings.t("settings.incall.roomLink", lang),
                            style = MaterialTheme.typography.labelSmall,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(onClick = { clipboardManager.setText(AnnotatedString(createdUrl!!)) }, modifier = Modifier.size(32.dp)) {
                            Icon(
                                Icons.Default.ContentCopy,
                                contentDescription = Strings.t("settings.incall.copied", lang),
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        IconButton(onClick = {
                            val sendIntent =
                                Intent().apply {
                                    action = Intent.ACTION_SEND
                                    putExtra(Intent.EXTRA_TEXT, createdUrl)
                                    type = "text/plain"
                                }
                            context.startActivity(Intent.createChooser(sendIntent, null))
                        }, modifier = Modifier.size(32.dp)) {
                            Icon(
                                Icons.Default.Share,
                                contentDescription = Strings.t("settings.incall.share", lang),
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                    OutlinedTextField(
                        value = createdUrl!!,
                        onValueChange = {},
                        readOnly = true,
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.fillMaxWidth(),
                    )

                    Spacer(Modifier.height(8.dp))

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Default.Smartphone, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(
                            Strings.t("settings.incall.deepLink", lang),
                            style = MaterialTheme.typography.labelSmall,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(onClick = { clipboardManager.setText(AnnotatedString(deepLink)) }, modifier = Modifier.size(32.dp)) {
                            Icon(
                                Icons.Default.ContentCopy,
                                contentDescription = Strings.t("settings.incall.copied", lang),
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        IconButton(onClick = {
                            val sendIntent =
                                Intent().apply {
                                    action = Intent.ACTION_SEND
                                    putExtra(Intent.EXTRA_TEXT, createdUrl)
                                    type = "text/plain"
                                }
                            context.startActivity(Intent.createChooser(sendIntent, null))
                        }, modifier = Modifier.size(32.dp)) {
                            Icon(
                                Icons.Default.Share,
                                contentDescription = Strings.t("settings.incall.share", lang),
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                    OutlinedTextField(
                        value = deepLink,
                        onValueChange = {},
                        readOnly = true,
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        },
        confirmButton = {
            if (createdUrl == null) {
                Button(
                    onClick = {
                        creating = true
                        error = null
                        coroutineScope.launch(Dispatchers.IO) {
                            try {
                                val result =
                                    VisioManager.client.createRoom(
                                        "https://$meetInstance",
                                        "",
                                        accessLevel,
                                    )
                                // Add accesses for invited users
                                if (accessLevel == "restricted") {
                                    for (user in invitedUsers) {
                                        try {
                                            VisioManager.client.addAccess(user.id, result.id)
                                        } catch (_: Exception) {
                                        }
                                    }
                                }
                                withContext(Dispatchers.Main) {
                                    createdRoomId = result.id
                                    createdUrl = "https://$meetInstance/${result.slug}"
                                    creating = false
                                }
                            } catch (e: Exception) {
                                withContext(Dispatchers.Main) {
                                    error = e.message ?: Strings.t("home.createRoom.error", lang)
                                    creating = false
                                }
                            }
                        }
                    },
                    enabled = !creating,
                ) {
                    Text(
                        if (creating) {
                            Strings.t("home.createRoom.creating", lang)
                        } else {
                            Strings.t("home.createRoom.create", lang)
                        },
                    )
                }
            } else {
                Button(onClick = { onCreated(createdUrl!!) }) {
                    Text(Strings.t("home.join", lang))
                }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(Strings.t("settings.cancel", lang))
            }
        },
    )
}

@Composable
private fun ServerPickerDialog(
    instances: List<String>,
    customServer: String,
    onCustomServerChange: (String) -> Unit,
    lang: String,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(Strings.t("home.serverPicker.title", lang)) },
        text = {
            Column {
                instances.forEach { instance ->
                    Text(
                        text = instance,
                        style = MaterialTheme.typography.bodyLarge,
                        color = VisioColors.Primary500,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clickable { onSelect(instance) }
                                .padding(vertical = 12.dp),
                    )
                }
                HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                OutlinedTextField(
                    value = customServer,
                    onValueChange = onCustomServerChange,
                    label = { Text(Strings.t("home.serverPicker.custom", lang)) },
                    placeholder = { Text("meet.example.com") },
                    singleLine = true,
                    keyboardOptions =
                        KeyboardOptions(
                            keyboardType = KeyboardType.Uri,
                            autoCorrectEnabled = false,
                            capitalization = KeyboardCapitalization.None,
                        ),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { if (customServer.isNotBlank()) onSelect(customServer.trim()) },
                enabled = customServer.isNotBlank(),
            ) {
                Text(Strings.t("home.connect", lang))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(Strings.t("home.serverPicker.cancel", lang))
            }
        },
    )
}
