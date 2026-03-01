package io.visio.mobile.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import io.visio.mobile.ui.CallScreen
import io.visio.mobile.ui.ChatScreen
import io.visio.mobile.ui.HomeScreen
import io.visio.mobile.ui.SettingsScreen
import java.net.URLDecoder
import java.net.URLEncoder

@Composable
fun AppNavigation() {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = "home") {

        composable("home") {
            HomeScreen(
                onJoin = { roomUrl, username ->
                    val encoded = URLEncoder.encode(roomUrl, "UTF-8")
                    val encodedName = URLEncoder.encode(username.ifBlank { "" }, "UTF-8")
                    navController.navigate("call/$encoded?username=$encodedName")
                },
                onSettings = {
                    navController.navigate("settings")
                }
            )
        }

        composable(
            route = "call/{roomUrl}?username={username}",
            arguments = listOf(
                navArgument("roomUrl") { type = NavType.StringType },
                navArgument("username") {
                    type = NavType.StringType
                    defaultValue = ""
                }
            )
        ) { backStackEntry ->
            val roomUrl = URLDecoder.decode(
                backStackEntry.arguments?.getString("roomUrl") ?: "",
                "UTF-8"
            )
            val username = URLDecoder.decode(
                backStackEntry.arguments?.getString("username") ?: "",
                "UTF-8"
            )
            CallScreen(
                roomUrl = roomUrl,
                username = username,
                onNavigateToChat = { navController.navigate("chat") },
                onHangUp = {
                    navController.popBackStack("home", inclusive = false)
                }
            )
        }

        composable("chat") {
            ChatScreen(
                onBack = { navController.popBackStack() }
            )
        }

        composable("settings") {
            SettingsScreen(
                onBack = { navController.popBackStack() }
            )
        }
    }
}
