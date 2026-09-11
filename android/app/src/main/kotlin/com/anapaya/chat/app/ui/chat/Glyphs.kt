// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * The screen's glyphs, drawn rather than imported.
 *
 * The path data is the design's own, so a stroke is the weight it was drawn at. An icon set would
 * bring a second stroke weight to a screen whose structure is carried by hairlines, and the brand
 * set has no menu or send glyph to take instead.
 */
private fun path(data: String): Path = PathParser().parsePathString(data).toPath()

/** Draws `path`, authored against a `viewBox` square, into whatever size the caller gave. */
private fun DrawScope.glyph(path: Path, viewBox: Float, colour: Color, stroke: Dp) {
    val factor = size.minDimension / viewBox
    scale(factor, pivot = Offset.Zero) {
        drawPath(
            path = path,
            color = colour,
            style = Stroke(
                width = stroke.toPx() / factor,
                cap = androidx.compose.ui.graphics.StrokeCap.Round,
                join = androidx.compose.ui.graphics.StrokeJoin.Round,
            ),
        )
    }
}

/** Three bars. The room list is behind them. */
@Composable
internal fun MenuGlyph(tint: Color, modifier: Modifier = Modifier) {
    Canvas(modifier = modifier.size(18.dp, 14.dp)) {
        val bar = 2.dp.toPx()
        val gap = 4.dp.toPx()
        repeat(3) { row ->
            drawRoundRect(
                color = tint,
                topLeft = Offset(0f, row * (bar + gap)),
                size = Size(size.width, bar),
                cornerRadius = androidx.compose.ui.geometry.CornerRadius(bar / 2),
            )
        }
    }
}

/** A plus. Makes a room. */
@Composable
internal fun PlusGlyph(tint: Color, modifier: Modifier = Modifier) {
    val cross = remember { path("M8 2.6v10.8M2.6 8h10.8") }
    Canvas(modifier = modifier.size(15.dp)) {
        glyph(cross, viewBox = 16f, colour = tint, stroke = 1.8.dp)
    }
}

/** An arrow to the right. Posts what was typed. */
@Composable
internal fun SendGlyph(tint: Color, modifier: Modifier = Modifier) {
    val arrow = remember { path("M3 10h13M10.5 4.5L16.5 10l-6 5.5") }
    Canvas(modifier = modifier.size(19.dp)) {
        glyph(arrow, viewBox = 20f, colour = tint, stroke = 1.9.dp)
    }
}

/** An arrow down. Returns to the newest message. */
@Composable
internal fun DownArrow(tint: Color, modifier: Modifier = Modifier) {
    val arrow = remember { path("M6 1v8.2M2.4 6.2L6 10l3.6-3.8") }
    Canvas(modifier = modifier.size(12.dp)) {
        glyph(arrow, viewBox = 12f, colour = tint, stroke = 1.6.dp)
    }
}

/** One turn every 700ms, while a call is out. */
@Composable
internal fun Spinner(tint: Color, modifier: Modifier = Modifier) {
    val turn = rememberInfiniteTransition(label = "spinner")
    val angle by turn.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(tween(700, easing = LinearEasing), RepeatMode.Restart),
        label = "angle",
    )

    Canvas(modifier = modifier.size(17.dp)) {
        val stroke = Stroke(width = 2.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round)
        val inset = stroke.width / 2
        drawCircle(color = tint.copy(alpha = 0.35f), radius = size.minDimension / 2 - inset, style = stroke)
        drawArc(
            color = tint,
            startAngle = angle,
            sweepAngle = 90f,
            useCenter = false,
            topLeft = Offset(inset, inset),
            size = Size(size.width - stroke.width, size.height - stroke.width),
            style = stroke,
        )
    }
}
