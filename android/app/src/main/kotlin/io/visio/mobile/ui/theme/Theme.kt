package io.visio.mobile.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
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

private val LightColorScheme = lightColorScheme(
    background = VisioColors.LightBackground,
    surface = VisioColors.LightSurface,
    surfaceVariant = VisioColors.LightSurfaceVariant,
    primary = VisioColors.Primary500,
    error = VisioColors.Error500,
    onBackground = VisioColors.LightOnBackground,
    onSurface = VisioColors.LightOnSurface,
    onPrimary = VisioColors.White,
    onError = VisioColors.White,
    outline = VisioColors.LightBorder,
)

@Composable
fun VisioTheme(darkTheme: Boolean = true, content: @Composable () -> Unit) {
    val colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme
    MaterialTheme(colorScheme = colorScheme, content = content)
}
