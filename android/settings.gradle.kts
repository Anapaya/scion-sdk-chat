// Copyright 2026 Anapaya Systems

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

/** How long the SDK's release is given to answer, and then to send. */
val CONNECT_TIMEOUT_MILLIS = 30_000
val READ_TIMEOUT_MILLIS = 120_000

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

    // A network that never answers should fail the build rather than hang it.
    fun read(from: String): ByteArray =
        java.net.URI(from).toURL().openConnection().run {
            connectTimeout = CONNECT_TIMEOUT_MILLIS
            readTimeout = READ_TIMEOUT_MILLIS
            getInputStream().use { it.readBytes() }
        }

    val bytes = read("$release/$archive")
    val published = String(read("$release/SHA256SUMS-android"))
        .lineSequence()
        .firstOrNull { it.endsWith(" $archive") }
        ?.substringBefore(' ')
        ?: error("the release publishes no checksum for $archive")
    val digest = java.security.MessageDigest.getInstance("SHA-256")
        .digest(bytes)
        .joinToString("") { "%02x".format(it) }
    check(digest == published) { "$archive does not match the checksum on its release" }

    // Unpacked beside the directory and moved into place whole. An unpack that throws partway
    // would otherwise leave a directory that every later build takes for a filled one.
    val staging = File(into.parentFile, "${into.name}.incomplete")
    staging.deleteRecursively()
    staging.mkdirs()

    try {
        java.util.zip.ZipInputStream(bytes.inputStream()).use { zip ->
            generateSequence { zip.nextEntry }.forEach { entry ->
                val file = staging.resolve(entry.name).normalize()
                // An archive may name a path outside the directory it is unpacked into.
                check(file.path.startsWith(staging.path)) { "${entry.name} escapes ${staging.path}" }

                if (entry.isDirectory) {
                    file.mkdirs()
                } else {
                    file.parentFile.mkdirs()
                    file.outputStream().use { zip.copyTo(it) }
                }
            }
        }
        check(staging.renameTo(into)) { "could not move ${staging.path} to ${into.path}" }
    } catch (failure: Throwable) {
        staging.deleteRecursively()
        throw failure
    }
}

// A Gradle root of its own, under a Cargo workspace. Keeping it here rather than at the repository
// root stops an IDE from trying to import the Rust crates as a Gradle project.
rootProject.name = "scion-sdk-chat-android"

include(":chat-client")
include(":app")
