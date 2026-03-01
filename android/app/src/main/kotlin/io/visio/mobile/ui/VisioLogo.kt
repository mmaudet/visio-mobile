package io.visio.mobile.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

// French flag colors
private val BlueFR = Color(0xFF002395)
private val WhiteFR = Color(0xFFFFFFFF)
private val RedFR = Color(0xFFED2939)

@Composable
fun VisioLogo(size: Dp = 64.dp) {
    Canvas(modifier = Modifier.size(size)) {
        val w = this.size.width
        val h = this.size.height

        drawTricoloreBackground(w, h)
        drawCameraBody(w, h)
        drawWifiArcs(w, h)
    }
}

private fun DrawScope.drawTricoloreBackground(w: Float, h: Float) {
    val third = w / 3f
    // Blue stripe
    drawRect(color = BlueFR, topLeft = Offset.Zero, size = Size(third, h))
    // White stripe
    drawRect(color = WhiteFR, topLeft = Offset(third, 0f), size = Size(third, h))
    // Red stripe
    drawRect(color = RedFR, topLeft = Offset(third * 2, 0f), size = Size(third, h))
}

private fun DrawScope.drawCameraBody(w: Float, h: Float) {
    val bodyColor = Color(0xFF1D1D1F)
    // Camera body rectangle
    val bodyLeft = w * 0.10f
    val bodyTop = h * 0.30f
    val bodyRight = w * 0.62f
    val bodyBottom = h * 0.70f
    drawRect(
        color = bodyColor,
        topLeft = Offset(bodyLeft, bodyTop),
        size = Size(bodyRight - bodyLeft, bodyBottom - bodyTop)
    )
    // Camera lens (trapezoid pointing right)
    val lensPath = Path().apply {
        moveTo(bodyRight, bodyTop + (bodyBottom - bodyTop) * 0.20f)
        lineTo(w * 0.78f, bodyTop)
        lineTo(w * 0.78f, bodyBottom)
        lineTo(bodyRight, bodyBottom - (bodyBottom - bodyTop) * 0.20f)
        close()
    }
    drawPath(lensPath, color = bodyColor)
    // Lens circle (recording indicator)
    val lensRadius = (bodyBottom - bodyTop) * 0.14f
    val lensCx = (bodyLeft + bodyRight) / 2f
    val lensCy = (bodyTop + bodyBottom) / 2f
    drawCircle(color = WhiteFR, radius = lensRadius, center = Offset(lensCx, lensCy))
}

private fun DrawScope.drawWifiArcs(w: Float, h: Float) {
    val arcColor = WhiteFR
    val strokeWidth = w * 0.035f
    val centerX = w * 0.82f
    val centerY = h * 0.55f

    // Three arcs (wifi signal)
    for (i in 1..3) {
        val radius = w * 0.06f * i
        drawArc(
            color = arcColor,
            startAngle = -135f,
            sweepAngle = 90f,
            useCenter = false,
            topLeft = Offset(centerX - radius, centerY - radius),
            size = Size(radius * 2, radius * 2),
            style = Stroke(width = strokeWidth, cap = StrokeCap.Round)
        )
    }

    // Dot at center
    drawCircle(
        color = arcColor,
        radius = strokeWidth * 0.8f,
        center = Offset(centerX, centerY)
    )
}
