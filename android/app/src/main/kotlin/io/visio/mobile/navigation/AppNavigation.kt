package io.visio.mobile.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import io.visio.mobile.VisioManager
import io.visio.mobile.ui.CallScreen
import io.visio.mobile.ui.ChatScreen
import io.visio.mobile.ui.HomeScreen
import io.visio.mobile.ui.PreJoinScreen
import io.visio.mobile.ui.SettingsScreen
import java.net.URLDecoder
import java.net.URLEncoder

@Composable
fun AppNavigation() {
    val navController = rememberNavController()

    // Auto-navigate to call screen for test deep links (debug only)
    LaunchedEffect(Unit) {
        if (VisioManager.pendingTestConnect != null) {
            // Navigate to call with a placeholder URL; CallScreen will use pendingTestConnect
            val encoded = URLEncoder.encode("test://direct-connect", "UTF-8")
            navController.navigate("call/$encoded?username=test-user&roomDisplayName=")
        }
    }

    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            HomeScreen(
                onJoin = { roomUrl, username, roomDisplayName ->
                    val encoded = URLEncoder.encode(roomUrl, "UTF-8")
                    val encodedName = URLEncoder.encode(username.ifBlank { "" }, "UTF-8")
                    val encodedDisplayName = URLEncoder.encode(roomDisplayName ?: "", "UTF-8")
                    navController.navigate("lobby/$encoded?username=$encodedName&roomDisplayName=$encodedDisplayName")
                },
                onSettings = {
                    navController.navigate("settings")
                },
            )
        }

        composable(
            route = "lobby/{roomUrl}?username={username}&roomDisplayName={roomDisplayName}",
            arguments =
                listOf(
                    navArgument("roomUrl") { type = NavType.StringType },
                    navArgument("username") {
                        type = NavType.StringType
                        defaultValue = ""
                    },
                    navArgument("roomDisplayName") {
                        type = NavType.StringType
                        defaultValue = ""
                    },
                ),
        ) { backStackEntry ->
            val roomUrl =
                URLDecoder.decode(
                    backStackEntry.arguments?.getString("roomUrl") ?: "",
                    "UTF-8",
                )
            val username =
                URLDecoder.decode(
                    backStackEntry.arguments?.getString("username") ?: "",
                    "UTF-8",
                )
            val roomDisplayName =
                URLDecoder.decode(
                    backStackEntry.arguments?.getString("roomDisplayName") ?: "",
                    "UTF-8",
                ).ifBlank { null }
            PreJoinScreen(
                roomUrl = roomUrl,
                initialUsername = username,
                roomDisplayName = roomDisplayName,
                onJoin = { finalUsername ->
                    val enc = URLEncoder.encode(roomUrl, "UTF-8")
                    val encName = URLEncoder.encode(finalUsername.ifBlank { "" }, "UTF-8")
                    val encDisplayName = URLEncoder.encode(roomDisplayName ?: "", "UTF-8")
                    navController.navigate("call/$enc?username=$encName&roomDisplayName=$encDisplayName") {
                        popUpTo("home")
                    }
                },
                onCancel = { navController.popBackStack() },
            )
        }

        composable(
            route = "call/{roomUrl}?username={username}&roomDisplayName={roomDisplayName}",
            arguments =
                listOf(
                    navArgument("roomUrl") { type = NavType.StringType },
                    navArgument("username") {
                        type = NavType.StringType
                        defaultValue = ""
                    },
                    navArgument("roomDisplayName") {
                        type = NavType.StringType
                        defaultValue = ""
                    },
                ),
        ) { backStackEntry ->
            val roomUrl =
                URLDecoder.decode(
                    backStackEntry.arguments?.getString("roomUrl") ?: "",
                    "UTF-8",
                )
            val username =
                URLDecoder.decode(
                    backStackEntry.arguments?.getString("username") ?: "",
                    "UTF-8",
                )
            val roomDisplayName =
                URLDecoder.decode(
                    backStackEntry.arguments?.getString("roomDisplayName") ?: "",
                    "UTF-8",
                ).ifBlank { null }
            CallScreen(
                roomUrl = roomUrl,
                username = username,
                roomDisplayName = roomDisplayName,
                onNavigateToChat = { navController.navigate("chat") },
                onHangUp = {
                    navController.popBackStack("home", inclusive = false)
                },
            )
        }

        composable("chat") {
            ChatScreen(
                onBack = { navController.popBackStack() },
            )
        }

        composable("settings") {
            SettingsScreen(
                onBack = { navController.popBackStack() },
            )
        }
    }
}
