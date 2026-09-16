// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/** How long a panel takes to arrive, and the curve it arrives on. */
public const val PanelMillis: Int = 220

public val PanelEasing: Easing = CubicBezierEasing(0.2f, 0.6f, 0.2f, 1f)

/** The roles Material 3 has no name for. */
@Immutable
public data class ChatPalette(
    /** Legible at small sizes, where the vivid blue is not. */
    val strongBlue: Color,
    /** Behind a message the reader wrote. */
    val ownBubble: Color,
    /** On top of [ownBubble], at 3.1:1. Below the 4.5:1 the small text asks for. */
    val onOwnBubble: Color,
    val accentTint: Color,
    /** The dot on a warning. Never the text: at 12.5sp orange on tint does not carry. */
    val risk: Color,
    val riskTint: Color,
    val riskBorder: Color,
    /** Around the composer's field, whose fill is too near the surface under it. */
    val fieldBorder: Color,
    val sendDisabled: Color,
    /** The transport badge's dot, while reads are arriving. */
    val live: Color,
)

private val LightPalette = ChatPalette(
    strongBlue = Color(0xFF165A97),
    ownBubble = Color(0xFF009CDE),
    onOwnBubble = Color(0xFFFFFFFF),
    accentTint = Color(0xFFE6F5FC),
    risk = Color(0xFFFF871F),
    riskTint = Color(0xFFFFEFE1),
    riskBorder = Color(0xFFFFD9B5),
    fieldBorder = Color(0xFFE0E6ED),
    sendDisabled = Color(0xFFB8C2CC),
    live = Color(0xFF6DBE45),
)

private val DarkPalette = ChatPalette(
    strongBlue = Color(0xFF5FC8F2),
    ownBubble = Color(0xFF009CDE),
    onOwnBubble = Color(0xFFFFFFFF),
    accentTint = Color(0x2E009CDE),
    risk = Color(0xFFFF871F),
    riskTint = Color(0xFF33261B),
    riskBorder = Color(0x61FF871F),
    fieldBorder = Color(0x24FFFFFF),
    sendDisabled = Color(0xFF4A5565),
    live = Color(0xFF7FD65A),
)

public val LocalChatPalette: ProvidableCompositionLocal<ChatPalette> =
    staticCompositionLocalOf { LightPalette }

/** The extra roles, beside [MaterialTheme.colorScheme]. */
public val chatPalette: ChatPalette
    @Composable get() = LocalChatPalette.current

private val Light = lightColorScheme(
    primary = Color(0xFF009CDE),
    onPrimary = Color(0xFFFFFFFF),
    background = Color(0xFFFFFFFF),
    onBackground = Color(0xFF1C1F2A),
    surface = Color(0xFFFFFFFF),
    onSurface = Color(0xFF1C1F2A),
    onSurfaceVariant = Color(0xFF5A6472),
    // The drawer, the sheet and the dialog.
    surfaceContainerLowest = Color(0xFFFFFFFF),
    // An incoming bubble, the composer's field and the dialog's.
    surfaceContainer = Color(0xFFF1F4F8),
    outlineVariant = Color(0xFFE6EBF0),
    outline = Color(0xFF8A97A6),
    error = Color(0xFFFF871F),
    onError = Color(0xFFFFFFFF),
    scrim = Color(0x731C1F2A),
)

private val Dark = darkColorScheme(
    primary = Color(0xFF009CDE),
    onPrimary = Color(0xFFFFFFFF),
    background = Color(0xFF1C1F2A),
    onBackground = Color(0xFFE9EEF4),
    surface = Color(0xFF1C1F2A),
    onSurface = Color(0xFFE9EEF4),
    onSurfaceVariant = Color(0xFFA7B4C3),
    surfaceContainerLowest = Color(0xFF242835),
    surfaceContainer = Color(0xFF2E3341),
    outlineVariant = Color(0x24FFFFFF),
    outline = Color(0xFF8A97A6),
    error = Color(0xFFFF871F),
    onError = Color(0xFFFFFFFF),
    scrim = Color(0x731C1F2A),
)

/** The app's colours, light and dark. Not dynamic: two clients of one room should look alike. */
@Composable
public fun ChatTheme(
    dark: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(LocalChatPalette provides if (dark) DarkPalette else LightPalette) {
        MaterialTheme(colorScheme = if (dark) Dark else Light, content = content)
    }
}
