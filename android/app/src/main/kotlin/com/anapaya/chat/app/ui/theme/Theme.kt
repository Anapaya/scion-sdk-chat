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

/** How long a panel takes to arrive, and the curve it arrives on. Calm: no bounce, no overshoot. */
public const val PanelMillis: Int = 220

public val PanelEasing: Easing = CubicBezierEasing(0.2f, 0.6f, 0.2f, 1f)

/**
 * The roles Material 3 has no name for.
 *
 * Blue carries emphasis — the open room, the reader's own messages, anything unread. Orange is kept
 * for risk, which here is the one thing that went wrong. Green is unused on this screen.
 */
@Immutable
public data class ChatPalette(
    /** Text and linework that must stay legible at small sizes, where the vivid blue does not. */
    val strongBlue: Color,
    /** Behind a message the reader wrote. */
    val ownBubble: Color,
    /** On top of [ownBubble]. */
    val onOwnBubble: Color,
    /** Behind the open room's row, and the drawer's own button. */
    val accentTint: Color,
    /** The dot on a warning. Never the text: at 12.5sp orange on tint does not carry. */
    val risk: Color,
    /** Behind a warning, and the hairline above it. */
    val riskTint: Color,
    val riskBorder: Color,
)

private val LightPalette = ChatPalette(
    strongBlue = Color(0xFF165A97),
    ownBubble = Color(0xFFCCEBF8),
    onOwnBubble = Color(0xFF12283F),
    accentTint = Color(0xFFE6F5FC),
    risk = Color(0xFFFF871F),
    riskTint = Color(0xFFFFEFE1),
    riskBorder = Color(0xFFFFD9B5),
)

private val DarkPalette = ChatPalette(
    strongBlue = Color(0xFF5FC8F2),
    ownBubble = Color(0xFF165A97),
    onOwnBubble = Color(0xFFFFFFFF),
    accentTint = Color(0x2E009CDE),
    risk = Color(0xFFFF871F),
    riskTint = Color(0xFF33261B),
    riskBorder = Color(0x61FF871F),
)

public val LocalChatPalette: ProvidableCompositionLocal<ChatPalette> =
    staticCompositionLocalOf { LightPalette }

/** The extra roles, beside [MaterialTheme.colorScheme]. */
public val chatPalette: ChatPalette
    @Composable get() = LocalChatPalette.current

private val Light = lightColorScheme(
    primary = Color(0xFF009CDE),
    onPrimary = Color(0xFFFFFFFF),
    background = Color(0xFFF7F9FB),
    onBackground = Color(0xFF1C1F2A),
    surface = Color(0xFFF7F9FB),
    onSurface = Color(0xFF1C1F2A),
    onSurfaceVariant = Color(0xFF4A5765),
    // Bubbles, the drawer, the sheet and the dialog all sit on this, above the page.
    surfaceContainerLowest = Color(0xFFFFFFFF),
    // The composer's field and the dialog's, which sit below it.
    surfaceContainer = Color(0xFFEEF2F6),
    outlineVariant = Color(0xFFDCE3EB),
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

/**
 * The app's colours, light and dark.
 *
 * Not dynamic colour: the phone is shown beside the terminal client, and a scheme taken from the
 * reader's wallpaper would leave the two with nothing in common.
 */
@Composable
public fun ChatTheme(
    dark: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(LocalChatPalette provides if (dark) DarkPalette else LightPalette) {
        MaterialTheme(colorScheme = if (dark) Dark else Light, content = content)
    }
}
