plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val defaultLlamaCppDir = rootProject.layout.projectDirectory.asFile
    .parentFile
    .parentFile
    .resolve("llama.cpp")
    .absolutePath
val llamaCppDir = providers.gradleProperty("llamaCppDir")
    .orElse(providers.environmentVariable("LLAMA_CPP_DIR"))
    .orElse(defaultLlamaCppDir)
    .get()
val llamaVersionFile = rootProject.layout.projectDirectory.file("llama.cpp.version").asFile
val expectedLlamaCppCommit = llamaVersionFile
    .readLines()
    .firstOrNull { it.startsWith("LLAMA_CPP_COMMIT=") }
    ?.substringAfter("=")
    ?.trim()
    ?: error("Missing LLAMA_CPP_COMMIT in ${llamaVersionFile.absolutePath}")

android {
    namespace = "dev.disobey.mango"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "dev.disobey.mango"
        minSdk = 28
        targetSdk = 36
        versionCode = 2
        versionName = "0.2.2"

        externalNativeBuild {
            cmake {
                arguments += listOf("-DLLAMA_CPP_DIR=$llamaCppDir")
            }
        }
    }

    signingConfigs {
        create("release") {
            val ksPath = System.getenv("KEYSTORE_PATH")
            if (!ksPath.isNullOrBlank()) {
                storeFile = file(ksPath)
                storePassword = System.getenv("KEYSTORE_PASSWORD") ?: ""
                keyAlias = System.getenv("KEY_ALIAS") ?: "mango"
                keyPassword = System.getenv("KEY_PASSWORD") ?: ""
            }
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".dev"
            versionNameSuffix = "-dev"
            ndk {
                // Local llama.cpp inference currently ships arm64 Android libs.
                abiFilters += listOf("arm64-v8a")
            }
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            ndk {
                // Release ships arm64-v8a only; x86_64 drops ~40% APK size
                // and no real device uses it. Emulators use the debug APK.
                abiFilters += listOf("arm64-v8a")
            }
            signingConfig = signingConfigs.getByName("release")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        resources.excludes.addAll(
            listOf("/META-INF/{AL2.0,LGPL2.1}", "META-INF/DEPENDENCIES"),
        )
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
        }
    }
}

tasks.register("ensureUniffiGenerated") {
    doLast {
        val out = file("src/main/java/dev/disobey/mango/rust/mango_core.kt")
        if (!out.exists()) {
            throw GradleException("Missing UniFFI Kotlin bindings. Run `just bindings-kotlin` first.")
        }
    }
}

tasks.register("verifyLlamaCppInputs") {
    doLast {
        val llamaDir = file(llamaCppDir)
        val missingHeaders = listOf(
            llamaDir.resolve("include/llama.h"),
            llamaDir.resolve("ggml/include/ggml.h"),
        ).filterNot { it.exists() }
        if (missingHeaders.isNotEmpty()) {
            throw GradleException(
                "Missing llama.cpp headers under $llamaDir. Set LLAMA_CPP_DIR or -PllamaCppDir to a llama.cpp checkout."
            )
        }
        val actualCommit = providers.exec {
            commandLine("git", "-C", llamaDir.absolutePath, "rev-parse", "HEAD")
        }.standardOutput.asText.get().trim()
        if (actualCommit != expectedLlamaCppCommit) {
            throw GradleException(
                "llama.cpp checkout at $llamaDir is not pinned to $expectedLlamaCppCommit (actual $actualCommit). Run `just fetch-llama-cpp`."
            )
        }

        val llamaLibDir = file("src/main/jniLibs/arm64-v8a")
        val missingLibs = listOf(
            "libggml-base.so",
            "libggml-cpu.so",
            "libggml.so",
            "libllama.so",
        )
            .map { llamaLibDir.resolve(it) }
            .filterNot { it.exists() }
        if (missingLibs.isNotEmpty()) {
            throw GradleException(
                "Missing llama.cpp Android libraries in $llamaLibDir. Run `just build-android` or set LLAMA_ANDROID_BIN before assembling."
            )
        }
    }
}

tasks.register("requireReleaseSigning") {
    doLast {
        val missing = listOf(
            "KEYSTORE_PATH",
            "KEYSTORE_PASSWORD",
            "KEY_ALIAS",
            "KEY_PASSWORD",
        ).filter { System.getenv(it).isNullOrBlank() }
        if (missing.isNotEmpty()) {
            throw GradleException(
                "Release signing is required for release builds. Missing environment variables: ${missing.joinToString(", ")}"
            )
        }
    }
}

tasks.named("preBuild") {
    dependsOn("ensureUniffiGenerated")
    dependsOn("verifyLlamaCppInputs")
}

tasks.matching {
    it.name == "assembleRelease" ||
        it.name == "bundleRelease" ||
        it.name == "packageRelease" ||
        it.name == "validateSigningRelease"
}.configureEach {
    dependsOn("requireReleaseSigning")
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.03.00")
    implementation(composeBom)

    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.security:security-crypto:1.1.0-alpha06")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.9.0")

    // Biometric authentication (Phase 28, D-22)
    implementation("androidx.biometric:biometric:1.4.0-alpha02")

    // AppCompat (Phase 28): required for FragmentActivity base class used by BiometricPrompt
    implementation("androidx.appcompat:appcompat:1.7.0")

    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")

    debugImplementation("androidx.compose.ui:ui-tooling")

    // UniFFI JNA
    implementation("net.java.dev.jna:jna:5.17.0@aar")

    // ONNX Runtime for on-device embedding inference (Phase 11, EMBD-03)
    implementation("com.microsoft.onnxruntime:onnxruntime-android:1.24.3")

    // Gson for parsing tokenizer.json vocabulary (Phase 11, EMBD-04)
    implementation("com.google.code.gson:gson:2.12.1")

    // Markdown rendering for chat messages
    implementation("com.mikepenz:multiplatform-markdown-renderer-m3:0.39.2")
    implementation("com.mikepenz:multiplatform-markdown-renderer-code:0.39.2")

    // Material Icons Extended for chat UI icons
    implementation("androidx.compose.material:material-icons-extended")

    // WorkManager for background agent execution (Phase 9, D-13)
    implementation("androidx.work:work-runtime-ktx:2.10.1")

    testImplementation("junit:junit:4.13.2")
}
