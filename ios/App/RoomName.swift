// Copyright 2026 Anapaya Systems

import Foundation

/// The longest room name this client will create. The server takes 64.
let roomNameMax = 10

/// Why `name` will not do, or `nil` when it will.
func roomNameProblem(_ name: String) -> String? {
    if name.isEmpty { return "a room needs a name" }
    if name.contains(where: \.isWhitespace) { return "a room name is one word" }
    if name.count > roomNameMax { return "room name is too long - \(roomNameMax) characters max" }
    return nil
}
