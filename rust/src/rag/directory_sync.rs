//! Phase 32: Directory-based RAG ingestion — walk + diff primitives.
//!
//! This module provides:
//!
//! - `diff_files` — cross-platform fingerprint diff (added / modified / removed)
//!   partition, consumed by the actor handler regardless of which platform
//!   produced the enumeration.
//! - `walk_with_exclusions` + `validate_exclusion_glob` — desktop-only
//!   enumeration and glob validation, powered by the `ignore` crate.
//! - `validate_glob_pattern` — cross-platform glob validation (works on iOS /
//!   Android) backed by `globset`, so UI code can validate user-entered
//!   exclusion globs over UniFFI without pulling the desktop-only `ignore`
//!   crate onto mobile targets.
//!
//! Requirements covered by this file (with tests in #[cfg(test)] mod tests):
//!   - DIR-01: incremental diff partitioning
//!   - DIR-02: exclusion-glob semantics + validation
//!
//! Threat model: walk_with_exclusions uses `ignore::OverrideBuilder` which
//! scopes exclusion patterns to the walk root — path-traversal globs like
//! `!../../etc/passwd` cannot escape. See `test_walk_path_traversal_inert`.

/// Result of comparing a stored directory-file fingerprint set against a
/// freshly-enumerated one.
///
/// Partitioning:
/// - `added`:    in `current`, not in `stored`
/// - `modified`: in both, but mtime or size differs
/// - `removed`:  in `stored`, not in `current`
/// - (unchanged files appear in NONE of the three buckets)
///
/// Each `(String, i64, i64)` tuple is `(file_path, mtime_secs, size_bytes)`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FileDiff {
    pub added: Vec<(String, i64, i64)>,
    pub modified: Vec<(String, i64, i64)>,
    pub removed: Vec<String>,
}

/// A stored directory-file fingerprint as read from `directory_files`.
///
/// Plan 32-02 keeps this definition local; once Plan 32-03 wires the actor
/// handler, it will either keep using this shape or adapt
/// `persistence::queries::DirectoryFileRow` into it.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFingerprint {
    pub file_path: String,
    pub mtime_secs: i64,
    pub size_bytes: i64,
}

/// Compare a stored fingerprint set against a fresh enumeration and return
/// the partitioned `FileDiff`.
///
/// `current` is `(path, mtime_secs, size_bytes)` as produced by
/// `walk_with_exclusions` on desktop or by native enumerators on mobile.
///
/// DIR-01: unchanged files (same path + mtime + size) are in NONE of the
/// buckets so callers can skip re-embedding them.
pub fn diff_files(stored: &[StoredFingerprint], current: &[(String, i64, i64)]) -> FileDiff {
    use std::collections::HashMap;
    let stored_map: HashMap<&str, (i64, i64)> = stored
        .iter()
        .map(|s| (s.file_path.as_str(), (s.mtime_secs, s.size_bytes)))
        .collect();
    let current_map: HashMap<&str, (i64, i64)> = current
        .iter()
        .map(|(p, m, s)| (p.as_str(), (*m, *s)))
        .collect();

    let mut diff = FileDiff::default();
    for (path, (mtime, size)) in &current_map {
        match stored_map.get(path) {
            None => diff.added.push(((*path).to_string(), *mtime, *size)),
            Some((sm, ss)) if *sm != *mtime || *ss != *size => {
                diff.modified.push(((*path).to_string(), *mtime, *size));
            }
            Some(_) => { /* unchanged — intentionally in no bucket */ }
        }
    }
    for s in stored {
        if !current_map.contains_key(s.file_path.as_str()) {
            diff.removed.push(s.file_path.clone());
        }
    }
    diff
}

/// Validate a user-supplied exclusion glob using `globset` — cross-platform.
///
/// Mobile UI calls this over UniFFI before persisting the glob so that
/// malformed patterns (unbalanced brackets, bad escapes, etc.) surface as
/// `Err` long before they ever reach the desktop walker. This is the
/// `T-32-V5b` mitigation.
pub fn validate_glob_pattern(glob: &str) -> anyhow::Result<()> {
    globset::Glob::new(glob)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("{}", e))
}

/// Desktop-only: walk `root` and return `(path, mtime_secs, size_bytes)` for
/// every file not matched by any exclusion glob.
///
/// Exclusion patterns follow `.gitignore`-style syntax via
/// `ignore::overrides::OverrideBuilder`; each pattern is added with a leading
/// `!` so that "matched" means "excluded" (OverrideBuilder defaults to an
/// allowlist — `!pat` inverts to a denylist entry).
///
/// Security (T-32-V5): `ignore` scopes exclusions to the walk root, so
/// traversal-style patterns like `../../etc/passwd` cannot escape.
/// `follow_links` defaults to `false`, preventing symlink escapes (T-32-V4).
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn walk_with_exclusions(
    root: &str,
    exclusion_globs: &[String],
) -> anyhow::Result<Vec<(String, i64, i64)>> {
    use ignore::{overrides::OverrideBuilder, WalkBuilder};
    use std::time::UNIX_EPOCH;

    let mut ob = OverrideBuilder::new(root);
    for glob in exclusion_globs {
        let pattern = if glob.starts_with('!') {
            glob.clone()
        } else {
            format!("!{}", glob)
        };
        ob.add(&pattern)
            .map_err(|e| anyhow::anyhow!("invalid exclusion glob '{}': {}", glob, e))?;
    }
    let overrides = ob
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build overrides: {}", e))?;

    let walker = WalkBuilder::new(root)
        .overrides(overrides)
        .hidden(false)
        .git_ignore(false)
        .standard_filters(false)
        .build();

    let mut out = Vec::new();
    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let size = meta.len() as i64;
        let path = entry.path().to_string_lossy().into_owned();
        out.push((path, mtime_secs, size));
    }
    Ok(out)
}

/// Desktop-only: validate an exclusion glob using `ignore::OverrideBuilder`.
///
/// This checks that the pattern is also accepted by the exact code path used
/// by `walk_with_exclusions`, in addition to the cross-platform `globset`
/// check in `validate_glob_pattern`.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn validate_exclusion_glob(glob: &str) -> anyhow::Result<()> {
    use ignore::overrides::OverrideBuilder;
    let mut ob = OverrideBuilder::new(".");
    let pattern = if glob.starts_with('!') {
        glob.to_string()
    } else {
        format!("!{}", glob)
    };
    ob.add(&pattern).map_err(|e| anyhow::anyhow!("{}", e))?;
    ob.build().map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(path: &str, mtime: i64, size: i64) -> StoredFingerprint {
        StoredFingerprint {
            file_path: path.to_string(),
            mtime_secs: mtime,
            size_bytes: size,
        }
    }

    // ---------- diff_files tests (DIR-01) ----------

    #[test]
    fn test_directory_diff_added_only() {
        let stored: Vec<StoredFingerprint> = vec![];
        let current = vec![("a.md".to_string(), 100, 10)];
        let diff = diff_files(&stored, &current);
        assert_eq!(diff.added, vec![("a.md".to_string(), 100, 10)]);
        assert!(diff.modified.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_directory_diff_removed_only() {
        let stored = vec![fp("a.md", 100, 10)];
        let current: Vec<(String, i64, i64)> = vec![];
        let diff = diff_files(&stored, &current);
        assert!(diff.added.is_empty());
        assert!(diff.modified.is_empty());
        assert_eq!(diff.removed, vec!["a.md".to_string()]);
    }

    #[test]
    fn test_directory_diff_modified_mtime() {
        let stored = vec![fp("a.md", 100, 10)];
        let current = vec![("a.md".to_string(), 200, 10)];
        let diff = diff_files(&stored, &current);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.modified, vec![("a.md".to_string(), 200, 10)]);
    }

    #[test]
    fn test_directory_diff_modified_size() {
        let stored = vec![fp("a.md", 100, 10)];
        let current = vec![("a.md".to_string(), 100, 99)];
        let diff = diff_files(&stored, &current);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.modified, vec![("a.md".to_string(), 100, 99)]);
    }

    #[test]
    fn test_directory_diff_unchanged() {
        let stored = vec![fp("a.md", 100, 10), fp("b.md", 200, 20)];
        let current = vec![("a.md".to_string(), 100, 10), ("b.md".to_string(), 200, 20)];
        let diff = diff_files(&stored, &current);
        assert!(
            diff.added.is_empty(),
            "unchanged files must not be in added"
        );
        assert!(
            diff.modified.is_empty(),
            "unchanged files must not be in modified"
        );
        assert!(
            diff.removed.is_empty(),
            "unchanged files must not be in removed"
        );
    }

    #[test]
    fn test_directory_diff_mixed() {
        let stored = vec![
            fp("unchanged.md", 100, 10), // stays
            fp("modified.md", 100, 10),  // mtime+size change
            fp("removed.md", 100, 10),   // disappears
        ];
        let current = vec![
            ("unchanged.md".to_string(), 100, 10),
            ("modified.md".to_string(), 200, 20),
            ("added.md".to_string(), 300, 30),
        ];
        let diff = diff_files(&stored, &current);
        assert_eq!(diff.added, vec![("added.md".to_string(), 300, 30)]);
        assert_eq!(diff.modified, vec![("modified.md".to_string(), 200, 20)]);
        assert_eq!(diff.removed, vec!["removed.md".to_string()]);
    }

    // ---------- cross-platform glob validation ----------

    #[test]
    fn test_validate_glob_pattern_ok() {
        assert!(validate_glob_pattern(".obsidian/").is_ok());
        assert!(validate_glob_pattern("*.tmp").is_ok());
        assert!(validate_glob_pattern("**/*.log").is_ok());
    }

    #[test]
    fn test_validate_glob_pattern_malformed() {
        // Unbalanced bracket — globset must reject.
        assert!(validate_glob_pattern("[abc").is_err());
    }

    // ---------- desktop-only walk + validate tests (DIR-02) ----------

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    mod desktop {
        use super::super::*;
        use std::fs;
        use std::path::Path;

        fn touch(root: &Path, rel: &str, body: &str) {
            let p = root.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&p, body).unwrap();
        }

        fn collect_paths(root: &Path, entries: &[(String, i64, i64)]) -> Vec<String> {
            // Return paths relative to `root` (forward-slash) for stable
            // assertions regardless of absolute tempdir location.
            let root_str = root.to_string_lossy().to_string();
            entries
                .iter()
                .map(|(p, _, _)| {
                    p.strip_prefix(&root_str)
                        .unwrap_or(p.as_str())
                        .trim_start_matches(std::path::MAIN_SEPARATOR)
                        .replace('\\', "/")
                        .to_string()
                })
                .collect()
        }

        #[test]
        fn test_walk_excludes_obsidian_dir() {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            touch(root, "note.md", "n");
            touch(root, ".obsidian/config.json", "{}");
            touch(root, "a/b.md", "x");

            let globs = vec![".obsidian/".to_string()];
            let result = walk_with_exclusions(root.to_str().unwrap(), &globs).unwrap();
            let rels = collect_paths(root, &result);

            assert!(rels.contains(&"note.md".to_string()), "got {:?}", rels);
            assert!(rels.contains(&"a/b.md".to_string()), "got {:?}", rels);
            assert!(
                !rels.iter().any(|p| p.contains(".obsidian")),
                ".obsidian dir leaked: {:?}",
                rels
            );
        }

        #[test]
        fn test_walk_excludes_tmp_glob() {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            touch(root, "scratch.tmp", "t");
            touch(root, "scratch.md", "m");

            let globs = vec!["*.tmp".to_string()];
            let result = walk_with_exclusions(root.to_str().unwrap(), &globs).unwrap();
            let rels = collect_paths(root, &result);

            assert!(rels.contains(&"scratch.md".to_string()), "got {:?}", rels);
            assert!(
                !rels.iter().any(|p| p.ends_with(".tmp")),
                "*.tmp leaked: {:?}",
                rels
            );
        }

        #[test]
        fn test_walk_no_exclusions_returns_all() {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            touch(root, "a.md", "a");
            touch(root, "sub/b.md", "b");

            let result = walk_with_exclusions(root.to_str().unwrap(), &[]).unwrap();
            let rels = collect_paths(root, &result);
            assert!(rels.contains(&"a.md".to_string()), "got {:?}", rels);
            assert!(rels.contains(&"sub/b.md".to_string()), "got {:?}", rels);
        }

        #[test]
        fn test_validate_exclusion_glob_ok() {
            assert!(validate_exclusion_glob(".obsidian/").is_ok());
            assert!(validate_exclusion_glob("*.tmp").is_ok());
            assert!(validate_exclusion_glob("**/*.log").is_ok());
        }

        #[test]
        fn test_validate_exclusion_glob_malformed() {
            // Unbalanced bracket — ignore's glob parser must reject.
            assert!(validate_exclusion_glob("[abc").is_err());
        }

        #[test]
        fn test_walk_path_traversal_inert() {
            // T-32-V5: a traversal-style exclusion must not cause the walker
            // to return files from outside the walk root. Regardless of
            // whether the pattern is honored or ignored, the walker is
            // scoped to `root` and cannot emit paths outside it.
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            touch(root, "inside.md", "i");

            let globs = vec!["../../etc/passwd".to_string()];
            let result = walk_with_exclusions(root.to_str().unwrap(), &globs).unwrap_or_default();

            let root_canon = root.canonicalize().unwrap();
            for (p, _, _) in &result {
                let canon = Path::new(p)
                    .canonicalize()
                    .unwrap_or_else(|_| Path::new(p).to_path_buf());
                assert!(
                    canon.starts_with(&root_canon),
                    "walk emitted path outside root: {:?} (root={:?})",
                    canon,
                    root_canon
                );
            }
        }
    }
}
