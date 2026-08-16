#[test]
fn wipe_guard_allows_android_files_dir() {
    let data_dir = "/data/user/0/dev.disobey.mango/files";
    assert!(crate::wipe_data_dir_allowed(
        data_dir,
        "/data/user/0/dev.disobey.mango/files/mango.db",
        "/data/user/0/dev.disobey.mango/files/mango_auth.db",
    ));
}

#[test]
fn wipe_guard_allows_android_files_dir_with_applicationid_suffix() {
    // Debug builds append `.dev` to the applicationId; beta builds may use
    // other suffixes. The wipe guard must accept all of them.
    for package in ["dev.disobey.mango.dev", "dev.disobey.mango.beta"] {
        let data_dir = format!("/data/user/0/{package}/files");
        assert!(
            crate::wipe_data_dir_allowed(
                &data_dir,
                &format!("/data/user/0/{package}/files/mango.db"),
                &format!("/data/user/0/{package}/files/mango_auth.db"),
            ),
            "wipe guard should accept suffixed package dir {package}"
        );
    }
}

#[test]
fn android_cache_cleanup_accepts_suffixed_package_dir() {
    use std::path::Path;
    let data_dir = Path::new("/data/user/0/dev.disobey.mango.dev/files");
    assert_eq!(
        crate::android_cache_dir_from_data_dir(data_dir),
        Some("/data/user/0/dev.disobey.mango.dev/cache".into())
    );
}

#[test]
fn wipe_guard_allows_ios_application_support_dir() {
    let data_dir = "/var/mobile/Containers/Data/Application/UUID/Library/Application Support";
    assert!(crate::wipe_data_dir_allowed(
        data_dir,
        "/var/mobile/Containers/Data/Application/UUID/Library/Application Support/mango.db",
        "/var/mobile/Containers/Data/Application/UUID/Library/Application Support/mango_auth.db",
    ));
}

#[test]
fn wipe_guard_rejects_unrelated_parent_dir() {
    assert!(!crate::wipe_data_dir_allowed(
        "/tmp",
        "/tmp/mango.db",
        "/tmp/mango_auth.db",
    ));
    assert!(!crate::wipe_data_dir_allowed(
        "/data/user/0/dev.disobey.mango/files",
        "/data/user/0/dev.disobey.mango/files/mango.db",
        "/data/user/0/other.app/files/mango_auth.db",
    ));
}

#[test]
fn fetch_url_rejects_local_and_private_targets_without_fetching() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    for url in [
        "file:///etc/passwd",
        "http://localhost:8080",
        "http://127.0.0.1:8080",
        "http://[::1]:8080",
        "http://[::ffff:127.0.0.1]:8080",
        "http://[::ffff:10.0.0.1]/",
        "http://[::ffff:169.254.169.254]/latest/meta-data/",
        "http://[::ffff:192.168.1.1]/",
        "http://169.254.169.254/latest/meta-data/",
        "http://192.168.1.1/",
    ] {
        let args = serde_json::json!({ "url": url }).to_string();
        let result = crate::agent::tools::dispatch_fetch_url(&args, &rt);
        assert!(
            result.starts_with("Error:"),
            "blocked URL should return an error for {url}; got {result:?}"
        );
    }
}

#[test]
fn plaintext_image_cleanup_only_removes_app_owned_sources() {
    let base = std::env::temp_dir().join(format!(
        "mango_cleanup_regression_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let app_data = base.join("files");
    let desktop_staging = app_data.join("image-attachments");
    let retry_dir = app_data.join("images/retry");
    let user_dir = base.join("Pictures");
    std::fs::create_dir_all(&desktop_staging).unwrap();
    std::fs::create_dir_all(&retry_dir).unwrap();
    std::fs::create_dir_all(&user_dir).unwrap();

    let user_image = user_dir.join("vacation.jpg");
    let staged_image = desktop_staging.join("desktop_probe.jpg");
    let retry_image = retry_dir.join("retry-probe.jpg");
    std::fs::write(&user_image, b"user-owned").unwrap();
    std::fs::write(&staged_image, b"staged").unwrap();
    std::fs::write(&retry_image, b"retry").unwrap();

    let data_dir = app_data.to_string_lossy().to_string();
    let user_image_path = user_image.to_string_lossy().to_string();
    let staged_image_path = staged_image.to_string_lossy().to_string();
    let retry_image_path = retry_image.to_string_lossy().to_string();
    crate::remove_plaintext_image_file(&user_image_path, &data_dir);
    crate::remove_plaintext_image_file(&staged_image_path, &data_dir);
    crate::remove_plaintext_image_file(&retry_image_path, &data_dir);

    assert!(
        user_image.exists(),
        "cleanup must not remove a user-owned desktop source image"
    );
    assert!(
        !staged_image.exists(),
        "cleanup should remove desktop app-owned staging images"
    );
    assert!(
        !retry_image.exists(),
        "cleanup should remove retry plaintext images under app data"
    );

    let android_root = base.join("android/data/user/0/dev.disobey.mango");
    let android_files = android_root.join("files");
    let android_cache = android_root.join("cache");
    std::fs::create_dir_all(&android_files).unwrap();
    std::fs::create_dir_all(&android_cache).unwrap();
    let android_image = android_cache.join("img_123.jpg");
    std::fs::write(&android_image, b"android-cache").unwrap();
    let android_image_path = android_image.to_string_lossy().to_string();
    let android_files_path = android_files.to_string_lossy().to_string();
    crate::remove_plaintext_image_file(&android_image_path, &android_files_path);
    assert!(
        !android_image.exists(),
        "cleanup should remove generated Android cache images"
    );

    let ios_container = base.join("var/mobile/Containers/Data/Application/UUID");
    let ios_app_support = ios_container.join("Library/Application Support");
    let ios_tmp = ios_container.join("tmp");
    std::fs::create_dir_all(&ios_app_support).unwrap();
    std::fs::create_dir_all(&ios_tmp).unwrap();
    let ios_image = ios_tmp.join("gallery_probe.jpg");
    std::fs::write(&ios_image, b"ios-tmp").unwrap();
    let ios_image_path = ios_image.to_string_lossy().to_string();
    let ios_app_support_path = ios_app_support.to_string_lossy().to_string();
    crate::remove_plaintext_image_file(&ios_image_path, &ios_app_support_path);
    assert!(
        !ios_image.exists(),
        "cleanup should remove generated iOS temporary images"
    );

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn dormant_inference_planner_is_not_exported_to_kotlin_bindings() {
    let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(|s| std::path::PathBuf::from(s).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let kotlin_path =
        workspace_root.join("android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt");
    let content = std::fs::read_to_string(&kotlin_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", kotlin_path.display()));

    for leaked_type in [
        "data class InferenceProfile",
        "enum class InferenceMode",
        "data class ResolvedRoute",
        "data class RouteTarget",
        "enum class RouteTargetRole",
    ] {
        assert!(
            !content.contains(leaked_type),
            "dormant inference planner type leaked into Kotlin bindings: {leaked_type}"
        );
    }
}
