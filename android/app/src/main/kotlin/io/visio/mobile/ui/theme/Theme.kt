package io.visio.mobile.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

private val DarkColorScheme = darkColorScheme(
    background = VisioColors.PrimaryDark50,
    surface = VisioColors.PrimaryDark75,
    surfaceVariant = VisioColors.PrimaryDark100,
    primary = VisioColors.Primary500,
    error = VisioColors.Error500,
    onBackground = VisioColors.White,
    onSurface = VisioColors.White,
    onPrimary = VisioColors.White,
    onError = VisioColors.White,
    outline = VisioColors.Greyscale400,
)

@Composable
fun VisioTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = DarkColorScheme, content = content)
}
