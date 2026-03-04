package io.visio.mobile.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.visio.mobile.R
import io.visio.mobile.ui.theme.VisioColors

@Composable
fun VisioLogo(size: Dp = 64.dp) {
    Image(
        painter = painterResource(R.mipmap.ic_launcher_foreground),
        contentDescription = "Visio Mobile",
        modifier = Modifier.size(size)
    )
}
