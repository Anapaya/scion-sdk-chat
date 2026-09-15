// Copyright 2026 Anapaya Systems

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

fetchScionSdk(file("libs/maven"))

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven { url = uri("libs/maven") }
    }
}

/**
 * Fills `into` from the SDK's GitHub release, once.
 *
 * The SDK is on no Maven repository, so Gradle cannot resolve it. The release carries the Maven
 * layout as one archive, and unpacking it keeps the POM, which brings the SDK's own dependencies
 * with it. Settings is where this runs: the repository has to hold the artifact before any project
 * resolves against it.
 */
fun fetchScionSdk(into: File) {
    if (into.isDirectory) return

    // The catalog is where the version is set. Settings cannot reach its accessors, so read it.
    val catalog = file("gradle/libs.versions.toml").readText()
    val version = Regex("""^scionSdk\s*=\s*"(.+)"$""", RegexOption.MULTILINE)
        .find(catalog)?.groupValues?.get(1)
        ?: error("gradle/libs.versions.toml declares no scionSdk version")

    val archive = "scion-http3-android-$version-maven.zip"
    val release = "https://github.com/Anapaya/scion-sdk/releases/download/v$version"
    println("Fetching the SCION SDK $version into ${into.path}")

    val bytes = java.net.URI("$release/$archive").toURL().readBytes()
    val published = java.net.URI("$release/SHA256SUMS-android").toURL().readText()
        .lineSequence()
        .firstOrNull { it.endsWith(" $archive") }
        ?.substringBefore(' ')
        ?: error("the release publishes no checksum for $archive")
    val digest = java.security.MessageDigest.getInstance("SHA-256")
        .digest(bytes)
        .joinToString("") { "%02x".format(it) }
    check(digest == published) { "$archive does not match the checksum on its release" }

    java.util.zip.ZipInputStream(bytes.inputStream()).use { zip ->
        generateSequence { zip.nextEntry }.forEach { entry ->
            val file = into.resolve(entry.name).normalize()
            // An archive may name a path outside the directory it is unpacked into.
            check(file.path.startsWith(into.path)) { "${entry.name} escapes ${into.path}" }

            if (entry.isDirectory) {
                file.mkdirs()
            } else {
                file.parentFile.mkdirs()
                file.outputStream().use { zip.copyTo(it) }
            }
        }
    }
}

// A Gradle root of its own, under a Cargo workspace. Keeping it here rather than at the repository
// root stops an IDE from trying to import the Rust crates as a Gradle project.
rootProject.name = "scion-sdk-chat-android"

include(":chat-client")
include(":app")
