// Copyright 2026 Anapaya Systems

import ChatClient
import Foundation

/// A row of the conversation: something somebody said, or the day it was said on.
enum ChatRow: Identifiable {
    case day(String, id: String)
    case said(Said)

    var id: String {
        switch self {
        case .day(_, let id): return id
        case .said(let said): return "seq \(said.message.seq)"
        }
    }

    /// One message, and where it sits in its author's run.
    struct Said {
        let message: Message
        let clock: String
        let mine: Bool
        let opensGroup: Bool
        let closesGroup: Bool
        /// A name under this message carries the gap, so the row leaves none of its own.
        let nameFollows: Bool
    }
}

/// The clock and the date. A fixed locale keeps it at 24 hours, as the terminal client prints it.
struct Clocks {
    private let time: DateFormatter
    private let date: DateFormatter
    private let calendar = Calendar.current
    private let now: () -> Date

    init(now: @escaping () -> Date = Date.init) {
        self.now = now
        time = DateFormatter()
        time.locale = Locale(identifier: "en_US_POSIX")
        time.dateFormat = "HH:mm"
        date = DateFormatter()
        date.locale = Locale(identifier: "en_US_POSIX")
        date.dateFormat = "d MMM"
    }

    func clock(_ at: Int64) -> String { time.string(from: stamp(at)) }

    /// Which day `at` falls on locally, as one comparable number.
    func dayOf(_ at: Int64) -> Int {
        Int(calendar.startOfDay(for: stamp(at)).timeIntervalSince1970)
    }

    /// What to write above the day's first message.
    func label(_ at: Int64) -> String {
        let today = calendar.startOfDay(for: now())
        let day = calendar.startOfDay(for: stamp(at))

        if day == today { return "TODAY" }
        if let yesterday = calendar.date(byAdding: .day, value: -1, to: today), day == yesterday {
            return "YESTERDAY"
        }
        return date.string(from: day).uppercased()
    }

    private func stamp(_ at: Int64) -> Date {
        Date(timeIntervalSince1970: TimeInterval(at) / 1000)
    }
}

/// The messages as rows, oldest first: the only order a change of day or of author is visible in.
func chatRows(messages: [Message], me: String?, clocks: Clocks) -> [ChatRow] {
    var rows: [ChatRow] = []
    var day: Int?

    for (index, message) in messages.enumerated() {
        let on = clocks.dayOf(message.postedAt)
        let newDay = on != day
        if newDay {
            day = on
            rows.append(.day(clocks.label(message.postedAt), id: "day \(on)"))
        }

        let before = index > 0 ? messages[index - 1] : nil
        let after = index + 1 < messages.count ? messages[index + 1] : nil
        // A day separator breaks a run, so the name is written again under it.
        let opens = newDay || before?.username != message.username
        let closes = after == nil
            || after?.username != message.username
            || clocks.dayOf(after!.postedAt) != on

        // A day separator carries its own space, so only a name on the same day counts.
        let nameFollows = closes
            && after != nil
            && after!.username != me
            && clocks.dayOf(after!.postedAt) == on

        rows.append(.said(ChatRow.Said(
            message: message,
            clock: clocks.clock(message.postedAt),
            mine: message.username == me,
            opensGroup: opens,
            closesGroup: closes,
            nameFollows: nameFollows)))
    }

    return rows
}
