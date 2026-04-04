plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "dev.disobey.mango"
    compileSdk = 35
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "dev.disobey.mango"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".dev"
            versionNameSuffix = "-dev"
        }
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
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

    kotlinOptions {
        jvmTarget = "17"
    }

    packaging {
        resources.excludes.addAll(
            listOf("/META-INF/{AL2.0,LGPL2.1}", "META-INF/DEPENDENCIES"),
        )
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
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

tasks.named("preBuild") {
    dependsOn("ensureUniffiGenerated")
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.04.01")
    implementation(composeBom)

    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.security:security-crypto:1.1.0-alpha06")
    implementation("androidx.activity:activity-compose:1.9.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.3")

    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")

    debugImplementation("androidx.compose.ui:ui-tooling")

    // UniFFI JNA
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    // ONNX Runtime for on-device embedding inference (Phase 11, EMBD-03)
    implementation("com.microsoft.onnxruntime:onnxruntime-android:1.24.3")

    // Gson for parsing tokenizer.json vocabulary (Phase 11, EMBD-04)
    implementation("com.google.code.gson:gson:2.10.1")

    // Markdown rendering for chat messages
    implementation("com.mikepenz:multiplatform-markdown-renderer-m3:0.35.0")
    implementation("com.mikepenz:multiplatform-markdown-renderer-code:0.35.0")

    // Material Icons Extended for chat UI icons
    implementation("androidx.compose.material:material-icons-extended")

    // WorkManager for background agent execution (Phase 9, D-13)
    implementation("androidx.work:work-runtime-ktx:2.9.1")
}
