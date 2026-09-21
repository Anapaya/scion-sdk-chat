#!/usr/bin/env bash
# Copyright 2026 Anapaya Systems
#
# Fills libs/ from the SDK's GitHub release, once. The SDK is on no Swift package registry, so
# SwiftPM cannot resolve it by name. sdk.version is the one place the version is set.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
into="$here/libs/scion-http3-swift"
[ -d "$into" ] && exit 0

version="$(tr -d '[:space:]' < "$here/sdk.version")"
archive="scion-http3-swift-$version.zip"
release="https://github.com/Anapaya/scion-sdk/releases/download/v$version"

echo "Fetching the SCION SDK $version into ios/libs"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl -fsSL "$release/$archive" -o "$work/$archive"
published="$(curl -fsSL "$release/SHA256SUMS-apple" | awk -v a="$archive" '$2 == a { print $1 }')"
[ -n "$published" ] || { echo "the release publishes no checksum for $archive" >&2; exit 1; }
actual="$(shasum -a 256 "$work/$archive" | awk '{ print $1 }')"
[ "$actual" = "$published" ] || { echo "$archive does not match the checksum on its release" >&2; exit 1; }

mkdir -p "$here/libs"
unzip -q "$work/$archive" -d "$here/libs"
