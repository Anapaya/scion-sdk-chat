#!/usr/bin/env bash
# Copyright 2026 Anapaya Systems
#
# Checks the code regions that the SCION SDK guide quotes. Each pair in docs/anchors.txt must name
# 1 region, and each marker in the repository must be in the list. See docs/README.md.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# The guide matches a marker as a substring of the first line that holds it. A name that is a
# prefix of another name in the same file would quote the wrong region, so that fails too.
{ git grep --untracked -n -E '^[[:space:]]*(//|#)[[:space:]]*ANCHOR(_END)?: ' -- ':!docs/' || true; } |
  awk -v list=docs/anchors.txt '
    function fail(message) {
      print "error: " message > "/dev/stderr"
      failures++
    }

    BEGIN {
      while ((getline line < list) > 0) {
        if (line ~ /^[[:space:]]*$/ || line ~ /^#/) continue
        cut = index(line, "#")
        if (cut == 0) {
          fail(list ": \"" line "\" is not <path>#<anchor>")
          continue
        }
        key = substr(line, 1, cut - 1) SUBSEP substr(line, cut + 1)
        listed[key] = 1
        order[++count] = key
      }
    }

    {
      # git grep prints <path>:<line>:<text>.
      cut = index($0, ":")
      path = substr($0, 1, cut - 1)
      rest = substr($0, cut + 1)
      cut = index(rest, ":")
      number = substr(rest, 1, cut - 1) + 0
      text = substr(rest, cut + 1)

      closing = text ~ /ANCHOR_END: /
      marker = closing ? "ANCHOR_END: " : "ANCHOR: "
      name = substr(text, index(text, marker) + length(marker))
      sub(/[[:space:]]+$/, "", name)

      key = path SUBSEP name
      found[key] = 1
      if (closing) {
        ends[key]++
        last[key] = number
      } else {
        starts[key]++
        first[key] = number
        names[path] = names[path] " " name
      }
    }

    END {
      for (i = 1; i <= count; i++) {
        key = order[i]
        split(key, part, SUBSEP)
        label = part[1] "#" part[2]
        if (system("test -f \"" part[1] "\"") != 0) {
          fail(label ": no such file")
        } else if (starts[key] != 1 || ends[key] != 1) {
          fail(label ": needs 1 ANCHOR and 1 ANCHOR_END, found " starts[key] + 0 " and " ends[key] + 0)
        } else if (last[key] <= first[key] + 1) {
          fail(label ": needs code between ANCHOR and ANCHOR_END")
        }
      }

      for (key in found) {
        if (!(key in listed)) {
          split(key, part, SUBSEP)
          fail(part[1] "#" part[2] ": not in " list)
        }
      }

      for (path in names) {
        n = split(names[path], word, " ")
        for (a = 1; a <= n; a++) {
          for (b = 1; b <= n; b++) {
            if (word[a] != word[b] && index(word[b], word[a]) == 1) {
              fail(path ": \"" word[a] "\" is a prefix of \"" word[b] "\"")
            }
          }
        }
      }

      if (failures > 0) exit 1
      print count " regions checked"
    }
  '
