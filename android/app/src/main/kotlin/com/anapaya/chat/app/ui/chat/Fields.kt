// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp

/**
 * A field with no decoration of its own.
 *
 * The design draws its own containers — a pill for the composer, a bordered box in the dialog — so
 * a Material text field would put a second one inside each of them.
 */
@Composable
internal fun PlainField(
    value: String,
    onValue: (String) -> Unit,
    placeholder: String,
    fontSize: TextUnit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
) {
    BasicTextField(
        value = value,
        onValueChange = onValue,
        enabled = enabled,
        singleLine = true,
        textStyle = LocalTextStyle.current.copy(
            fontSize = fontSize,
            color = MaterialTheme.colorScheme.onSurface,
        ),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        modifier = modifier,
        decorationBox = { field ->
            Box(contentAlignment = Alignment.CenterStart) {
                if (value.isEmpty()) {
                    Text(
                        text = placeholder,
                        fontSize = fontSize,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                field()
            }
        },
    )
}

/**
 * A one-pixel rule along the top edge.
 *
 * Structure on this screen is carried by hairlines and space rather than elevation, so these appear
 * wherever two bands meet.
 */
internal fun Modifier.drawTopHairline(colour: Color): Modifier = drawBehind {
    val thickness = 1.dp.toPx()
    drawLine(
        color = colour,
        start = Offset(0f, thickness / 2),
        end = Offset(size.width, thickness / 2),
        strokeWidth = thickness,
    )
}

/** As [drawTopHairline], along the bottom. */
internal fun Modifier.drawBottomHairline(colour: Color): Modifier = drawBehind {
    val thickness = 1.dp.toPx()
    drawLine(
        color = colour,
        start = Offset(0f, size.height - thickness / 2),
        end = Offset(size.width, size.height - thickness / 2),
        strokeWidth = thickness,
    )
}
