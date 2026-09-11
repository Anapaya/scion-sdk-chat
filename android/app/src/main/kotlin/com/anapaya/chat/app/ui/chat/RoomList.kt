// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.anapaya.chat.app.ui.theme.chatPalette
import com.anapaya.chat.client.model.Room

/** How wide the room list is, drawn over the conversation or beside it. */
internal val DRAWER_WIDTH = 296.dp

private val ROW_SHAPE = RoundedCornerShape(percent = 50)

/**
 * Every room, and the way to make another.
 *
 * The body of both drawers. Which one wraps it is [ChatScreen]'s business, so the list is written
 * once.
 */
@Composable
internal fun RoomList(
    rooms: List<Room>,
    openRoomId: Long?,
    unread: Set<Long>,
    pending: Boolean,
    onOpenRoom: (Room) -> Unit,
    onCreate: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Header(rooms.size, unread.size)

        NewRoom(enabled = !pending, onClick = onCreate)

        LazyColumn(
            modifier = Modifier.padding(top = 8.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            items(rooms, key = { it.id }) { room ->
                RoomRow(
                    room = room,
                    selected = room.id == openRoomId,
                    unread = room.id in unread,
                    enabled = !pending,
                    onClick = { onOpenRoom(room) },
                )
            }
        }
    }
}

@Composable
private fun Header(rooms: Int, unread: Int) {
    Column(modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 12.dp)) {
        Text("Rooms", fontSize = 19.sp, fontWeight = FontWeight.SemiBold)
        Text(
            text = buildString {
                append(rooms)
                append(if (rooms == 1) " room" else " rooms")
                append(if (unread == 0) " · all caught up" else " · $unread with unread")
            },
            fontSize = 11.5.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 2.dp),
        )
    }
}

@Composable
private fun NewRoom(enabled: Boolean, onClick: () -> Unit) {
    val palette = chatPalette

    Row(
        modifier = Modifier
            .padding(horizontal = 12.dp)
            .fillMaxWidth()
            .height(40.dp)
            .background(palette.accentTint, ROW_SHAPE)
            .clickable(enabled = enabled, onClick = onClick)
            .alpha(if (enabled) 1f else 0.38f),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PlusGlyph(tint = palette.strongBlue)
        Text(
            text = "New room",
            fontSize = 13.5.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.strongBlue,
            modifier = Modifier.padding(start = 8.dp),
        )
    }
}

@Composable
private fun RoomRow(
    room: Room,
    selected: Boolean,
    unread: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val palette = chatPalette

    Row(
        modifier = Modifier
            .padding(horizontal = 8.dp)
            .fillMaxWidth()
            .height(50.dp)
            .background(if (selected) palette.accentTint else MaterialTheme.colorScheme.surfaceContainerLowest, ROW_SHAPE)
            .clickable(enabled = enabled, onClick = onClick)
            .alpha(if (enabled) 1f else 0.38f)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = "#${room.name}",
            fontSize = 14.5.sp,
            // Unread carries in the weight as well as the marker, so the row reads at a glance
            // without being read. The open room is never unread, so the two cannot collide.
            fontWeight = if (selected || unread) FontWeight.SemiBold else FontWeight.Normal,
            color = if (selected) palette.strongBlue else MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )

        // A dot, never a number. `seq` is assigned server-wide, so the distance between two of them
        // counts other rooms' messages and would overstate this one's.
        if (unread) {
            Box(
                modifier = Modifier
                    .padding(start = 8.dp)
                    .size(9.dp)
                    .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(percent = 50)),
            )
        }
    }
}
