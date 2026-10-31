//! The storage adapters against real files and a real SQLite database.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use klaub_core::config::Config;
use klaub_core::ids::{ReviewId, SnapshotId, WorktreeId};
use klaub_core::ports::{
    AnalysisCache, CacheKey, ConfigSource, FileReader, FlowRepository, ReviewRepository,
};
use klaub_core::review::{Brief, REVIEW_SCHEMA_VERSION, Review, ReviewStatus};
use klaub_core::snapshot::Scope;
use klaub_core::timestamp::Timestamp;
use klaub_storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, NoCache, SqliteCache,
};

fn review(id: &str, worktree: &str, updated: i64) -> Review {
    Review {
        schema_version: REVIEW_SCHEMA_VERSION,
        review_id: ReviewId::parse(id).unwrap(),
        previous_review_id: None,
        session_id: None,
        repository: "/r".into(),
        repository_id: "r".into(),
        worktree_id: WorktreeId::parse(worktree).unwrap(),
        base: "HEAD".into(),
        base_oid: "abc".into(),
        scope: Scope::Worktree,
        snapshot_id: SnapshotId::parse("snap_one").unwrap(),
        created_at: Timestamp::from_unix(1).unwrap(),
        updated_at: Timestamp::from_unix(updated).unwrap(),
        status: ReviewStatus::Draft,
        brief: Brief::default(),
        comments: vec![],
        reviewed_units: vec![],
        last_unit: None,
        feedback: None,
        flow: None,
    }
}

fn root() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    (dir, root)
}

#[test]
fn reviews_round_trip_and_are_listed_newest_first() {
    let (_guard, root) = root();
    let repo = JsonReviewRepository::new(&root);
    assert!(repo.list().unwrap().is_empty());
    repo.save(&review("rev_old", "wt_a", 10)).unwrap();
    repo.save(&review("rev_new", "wt_a", 20)).unwrap();
    repo.save(&review("rev_other", "wt_b", 30)).unwrap();

    let loaded = repo.load(&ReviewId::parse("rev_old").unwrap()).unwrap();
    assert_eq!(loaded, review("rev_old", "wt_a", 10));
    let listed: Vec<String> = repo
        .list()
        .unwrap()
        .iter()
        .map(|s| s.review_id.to_string())
        .collect();
    assert_eq!(listed, vec!["rev_other", "rev_new", "rev_old"]);
    assert_eq!(
        repo.latest(&WorktreeId::parse("wt_a").unwrap())
            .unwrap()
            .unwrap()
            .as_str(),
        "rev_new"
    );
    assert_eq!(
        repo.latest(&WorktreeId::parse("wt_zzz").unwrap()).unwrap(),
        None
    );
    assert!(matches!(
        repo.load(&ReviewId::parse("rev_missing").unwrap()),
        Err(klaub_core::Error::NotFound { .. })
    ));
}

#[test]
fn files_are_private_pretty_and_in_the_documented_place() {
    let (_guard, root) = root();
    let repo = JsonReviewRepository::new(&root);
    repo.save(&review("rev_one", "wt_a", 1)).unwrap();
    let path = root.join(".klaub/reviews/rev_one.json");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(root.join(".klaub/reviews"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.ends_with('\n') && text.contains("\"schema_version\": 1"),
        "{text}"
    );
    assert_eq!(
        JsonReviewRepository::relative_path(&ReviewId::parse("rev_one").unwrap()),
        ".klaub/reviews/rev_one.json"
    );
}

#[test]
fn unknown_fields_are_ignored_and_other_schema_versions_refused() {
    let (_guard, root) = root();
    let repo = JsonReviewRepository::new(&root);
    repo.save(&review("rev_one", "wt_a", 1)).unwrap();
    let path = root.join(".klaub/reviews/rev_one.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    value["added_by_a_newer_tool"] = serde_json::json!({"anything": [1, 2, 3]});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        repo.load(&ReviewId::parse("rev_one").unwrap()).is_ok(),
        "forward compatible"
    );

    value["schema_version"] = serde_json::json!(2);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = repo
        .load(&ReviewId::parse("rev_one").unwrap())
        .unwrap_err()
        .to_string();
    assert!(error.contains("schema_version"), "{error}");
    assert!(
        repo.list().unwrap().is_empty(),
        "an unreadable review does not break listing"
    );
}

#[test]
fn a_symlinked_klaub_directory_is_never_followed() {
    let (_guard, root) = root();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join(".klaub")).unwrap();
    let repo = JsonReviewRepository::new(&root);
    assert!(repo.save(&review("rev_one", "wt_a", 1)).is_err());
    assert_eq!(
        fs::read_dir(outside.path()).unwrap().count(),
        0,
        "nothing was written outside the worktree"
    );
    assert!(repo.list().is_err());
}

#[test]
fn concurrent_saves_never_corrupt_a_review() {
    let (_guard, root) = root();
    let repo = Arc::new(JsonReviewRepository::new(&root));
    let handles: Vec<_> = (0..8)
        .map(|n| {
            let repo = Arc::clone(&repo);
            std::thread::spawn(move || {
                for round in 0..20 {
                    repo.save(&review("rev_shared", "wt_a", n * 100 + round))
                        .unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let loaded = repo.load(&ReviewId::parse("rev_shared").unwrap()).unwrap();
    assert_eq!(loaded.review_id.as_str(), "rev_shared");
    let stray: Vec<_> = fs::read_dir(root.join(".klaub/reviews"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e == "tmp")
        })
        .collect();
    assert!(stray.is_empty(), "{stray:?}");
}

#[test]
fn flow_files_are_read_with_limits_and_without_following_links() {
    let (_guard, root) = root();
    let repo = FsFlowRepository::new(&root, ".klaub/flows").unwrap();
    let snapshot = SnapshotId::parse("snap_abc123").unwrap();
    assert_eq!(
        repo.expected_path(&snapshot).unwrap().as_str(),
        ".klaub/flows/snap_abc123.json"
    );
    assert_eq!(
        repo.read(&snapshot, 1024).unwrap(),
        None,
        "missing is not an error"
    );

    fs::create_dir_all(root.join(".klaub/flows")).unwrap();
    fs::write(root.join(".klaub/flows/snap_abc123.json"), b"{\"k\":1}").unwrap();
    let file = repo.read(&snapshot, 1024).unwrap().unwrap();
    assert_eq!(file.bytes, b"{\"k\":1}");
    assert!(repo.read(&snapshot, 3).is_err(), "too large");

    let secret = tempfile::NamedTempFile::new().unwrap();
    let other = SnapshotId::parse("snap_link").unwrap();
    std::os::unix::fs::symlink(secret.path(), root.join(".klaub/flows/snap_link.json")).unwrap();
    assert!(
        repo.read(&other, 1024).is_err(),
        "a link to a file elsewhere is refused"
    );
    assert!(FsFlowRepository::new(&root, "../outside").is_err());
}

#[test]
fn the_file_reader_stays_inside_the_worktree() {
    let (_guard, root) = root();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/brief.md"), "# brief").unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("docs/link.md")).unwrap();

    let reader = FsFileReader;
    assert_eq!(
        reader
            .read_bounded(&root, std::path::Path::new("docs/brief.md"), 100)
            .unwrap(),
        b"# brief"
    );
    assert_eq!(
        reader
            .read_bounded(&root, &root.join("docs/brief.md"), 100)
            .unwrap(),
        b"# brief",
        "absolute paths inside are fine"
    );
    assert!(
        reader
            .read_bounded(&root, std::path::Path::new("docs/link.md"), 100)
            .is_err(),
        "symlink escape"
    );
    assert!(
        reader.read_bounded(&root, outside.path(), 100).is_err(),
        "absolute path outside"
    );
    assert!(
        reader
            .read_bounded(&root, std::path::Path::new("../../etc/passwd"), 100)
            .is_err()
    );
    assert!(
        reader
            .read_bounded(&root, std::path::Path::new("docs/brief.md"), 3)
            .is_err(),
        "size limit"
    );
    assert!(
        reader
            .modified(&root, std::path::Path::new("docs/brief.md"))
            .unwrap()
            .is_some()
    );
}

fn key(worktree: &str, hash: &str, version: u32) -> CacheKey {
    CacheKey {
        worktree: WorktreeId::parse(worktree).unwrap(),
        content_hash: hash.into(),
        parser_version: version,
        kind: "parsed_file",
    }
}

#[test]
fn the_cache_is_keyed_by_content_and_parser_version() {
    let temp = tempfile::tempdir().unwrap();
    let cache = SqliteCache::open(&temp.path().join("cache/analysis.sqlite3")).unwrap();
    assert!(cache.is_empty().unwrap());
    cache.put(&key("wt_a", "h1", 1), b"v1").unwrap();
    assert_eq!(
        cache.get(&key("wt_a", "h1", 1)).unwrap().as_deref(),
        Some(&b"v1"[..])
    );
    assert_eq!(
        cache.get(&key("wt_a", "h1", 2)).unwrap(),
        None,
        "another parser version never matches"
    );
    assert_eq!(cache.get(&key("wt_b", "h1", 1)).unwrap(), None);
    assert_eq!(cache.get(&key("wt_a", "h2", 1)).unwrap(), None);
    cache.put(&key("wt_a", "h1", 1), b"v2").unwrap();
    assert_eq!(
        cache.get(&key("wt_a", "h1", 1)).unwrap().as_deref(),
        Some(&b"v2"[..])
    );
    assert_eq!(cache.len().unwrap(), 1);

    // It survives reopening.
    drop(cache);
    let again = SqliteCache::open(&temp.path().join("cache/analysis.sqlite3")).unwrap();
    assert_eq!(
        again.get(&key("wt_a", "h1", 1)).unwrap().as_deref(),
        Some(&b"v2"[..])
    );
}

#[test]
fn checking_a_cache_looks_without_touching() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("analysis.sqlite3");
    let cache = SqliteCache::open(&path).unwrap();
    cache.put(&key("wt_a", "h1", 1), b"v1").unwrap();
    drop(cache);

    let health = SqliteCache::check(&path).unwrap();
    assert_eq!(health.entries, 1);
    assert!(health.is_healthy(), "{health:?}");

    // A damaged file is reported, and left exactly as it was: only `open` replaces it.
    let garbage = temp.path().join("damaged.sqlite3");
    let bytes = b"this is definitely not a sqlite database, just text padding ".repeat(100);
    fs::write(&garbage, &bytes).unwrap();
    assert!(SqliteCache::check(&garbage).is_err());
    assert_eq!(fs::read(&garbage).unwrap(), bytes, "check must not repair");

    // A file that does not exist is not created by looking at it.
    let missing = temp.path().join("absent.sqlite3");
    assert!(SqliteCache::check(&missing).is_err());
    assert!(!missing.exists());
}

#[test]
fn a_corrupt_cache_is_replaced_not_reported() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("analysis.sqlite3");
    fs::write(
        &path,
        b"this is definitely not a sqlite database, just text padding ".repeat(100),
    )
    .unwrap();
    let cache = SqliteCache::open(&path).unwrap();
    cache.put(&key("wt_a", "h", 1), b"x").unwrap();
    assert!(cache.get(&key("wt_a", "h", 1)).unwrap().is_some());
}

#[test]
fn the_null_cache_and_the_in_memory_cache_are_drop_in_replacements() {
    let none: &dyn AnalysisCache = &NoCache;
    none.put(&key("wt_a", "h", 1), b"x").unwrap();
    assert_eq!(none.get(&key("wt_a", "h", 1)).unwrap(), None);
    let memory = SqliteCache::in_memory().unwrap();
    memory.put(&key("wt_a", "h", 1), b"x").unwrap();
    assert_eq!(memory.len().unwrap(), 1);
}

#[test]
fn configuration_layers_user_then_project() {
    let (_guard, root) = root();
    let user_dir = tempfile::tempdir().unwrap();
    let user = user_dir.path().join("config.toml");
    fs::write(
        &user,
        "[ui]\ntheme = \"light\"\nshow_hints = false\n[review]\nbase = \"main\"\n",
    )
    .unwrap();
    fs::write(
        root.join(".klaub.toml"),
        "[ui]\ntheme = \"high_contrast\"\n",
    )
    .unwrap();

    let config = FsConfigSource::new(Some(user.clone())).load(&root).unwrap();
    assert_eq!(config.ui.theme, klaub_core::config::Theme::HighContrast);
    assert!(!config.ui.show_hints);
    assert_eq!(config.review.base, "main");

    assert_eq!(
        FsConfigSource::new(None).load(&root).unwrap().review.base,
        "HEAD"
    );
    let (_other_guard, empty) = self::root();
    assert_eq!(
        FsConfigSource::new(None).load(&empty).unwrap(),
        Config::default()
    );
}

#[test]
fn configuration_errors_name_the_file_and_position() {
    let (_guard, root) = root();
    fs::write(
        root.join(".klaub.toml"),
        "[review]\nbase = \"HEAD\"\ntypo_key = 1\n",
    )
    .unwrap();
    let error = FsConfigSource::new(None)
        .load(&root)
        .unwrap_err()
        .to_string();
    assert!(error.contains(".klaub.toml:3:1"), "{error}");

    fs::write(
        root.join(".klaub.toml"),
        "[analysis]\nmatch_probable_threshold = 0.99\nmatch_strong_threshold = 0.5\n",
    )
    .unwrap();
    assert!(
        FsConfigSource::new(None)
            .load(&root)
            .unwrap_err()
            .to_string()
            .contains("must not exceed")
    );
    fs::remove_file(root.join(".klaub.toml")).unwrap();
    fs::create_dir(root.join(".klaub.toml")).unwrap();
    assert!(
        FsConfigSource::new(None).load(&root).is_err(),
        "a directory is not a configuration file"
    );
}
