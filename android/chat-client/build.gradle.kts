// Copyright 2026 Anapaya Systems

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.anapaya.chat.client"
    compileSdk = 35

    // Pinned for the same reason the SDK's own modules pin it: AGP's default for this version is
    // 34.0.0, and letting it choose downloads a second copy of the build tools.
    buildToolsVersion = "35.0.0"

    defaultConfig {
        minSdk = 24
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    // The one module that may depend on the SDK. `app` depends on this, never on the SDK, so the
    // UI cannot reach SCION even by accident.
    api(libs.scion.http3)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.kotlinx.serialization.json)

    testImplementation(kotlin("test"))
    testImplementation(libs.kotlinx.coroutines.test)
}

// The SDK is published to no public repository, so a missing directory is the first thing a reader
// hits. Say what to run rather than let Gradle report an unresolved dependency.
val checkSdkArtifact by tasks.registering {
    val repository = rootProject.layout.projectDirectory.dir("libs/maven").asFile
    doFirst {
        check(repository.isDirectory) {
            """
            The SCION SDK is not in android/libs/maven. From the repository root:

                gh release download v0.7.0 --repo Anapaya/scion-sdk -p 'scion-http3-android-*-maven.zip'
                unzip -q scion-http3-android-0.7.0-maven.zip -d android/libs/maven
            """.trimIndent()
        }
    }
}

tasks.named("preBuild") { dependsOn(checkSdkArtifact) }
