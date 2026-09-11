// Copyright 2026 Anapaya Systems

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // The SCION SDK, which is published to no public repository. `android/README.md` has the
        // one command that fills this directory; `checkSdkArtifact` repeats it when it is empty.
        maven { url = uri("libs/maven") }
    }
}

// A Gradle root of its own, under a Cargo workspace. Keeping it here rather than at the repository
// root stops an IDE from trying to import the Rust crates as a Gradle project.
rootProject.name = "scion-sdk-chat-android"

include(":chat-client")
include(":app")
