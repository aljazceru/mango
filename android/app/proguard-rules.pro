# JNA (used by UniFFI for Rust FFI)
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn com.sun.jna.**

# UniFFI generated Kotlin bindings
-keep class dev.disobey.mango.rust.** { *; }
-keepclassmembers class dev.disobey.mango.rust.** { *; }

# Kotlin coroutines
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}

# ONNX Runtime
-keep class ai.onnxruntime.** { *; }
-dontwarn ai.onnxruntime.**

# WorkManager
-keep class * extends androidx.work.Worker {}
-keep class * extends androidx.work.ListenableWorker { public <init>(...); }
-keep class androidx.work.impl.** { *; }

# Tink crypto — javax annotations not present at runtime
-dontwarn javax.annotation.Nullable
-dontwarn javax.annotation.concurrent.GuardedBy
