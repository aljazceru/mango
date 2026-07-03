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
