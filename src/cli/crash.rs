//! Local crash reports.
//!
//! When Klaub panics it writes a short text file under its state directory and then lets the normal
//! panic message through. Nothing is sent anywhere and nothing is uploaded: a report leaves the
//! machine only when the user prints it with `klaub crashes show` and passes it on themselves.
//!
//! A report holds the version, the platform, the subcommand name (not its arguments, which can be
//! paths), the panic message cut to a few hundred characters, and where in Klaub's own source the
//! panic happened. It never holds file contents, comments or the repository path, and the home
//! directory is written as `~`. The panic message can still quote data that caused the failure, so
//! `show` says to read the report before sharing it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::domain::text::sanitize_single_line;

/// Longest panic message kept.
const MAX_MESSAGE: usize = 400;

/// At most this many reports are kept; the oldest are removed when a new one is written.
const KEEP: usize = 20;

/// What a report is made from.
pub(crate) struct Report<'a> {
    pub(crate) version: &'a str,
    pub(crate) platform: &'a str,
    pub(crate) unix_time: u64,
    pub(crate) command: &'a str,
    pub(crate) message: &'a str,
    pub(crate) location: &'a str,
    pub(crate) home: Option<&'a Path>,
}

/// Replaces the home directory with `~` anywhere in `text`.
fn without_home(text: &str, home: Option<&Path>) -> String {
    match home.and_then(Path::to_str).filter(|h| h.len() > 1) {
        Some(home) => text.replace(home, "~"),
        None => text.to_owned(),
    }
}

/// Renders a report as the text that is written to disk.
pub(crate) fn render(report: &Report<'_>) -> String {
    let message: String = sanitize_single_line(report.message)
        .chars()
        .take(MAX_MESSAGE)
        .collect();
    let mut out = String::new();
    let _ = writeln!(out, "klaub crash report");
    let _ = writeln!(out, "version:  {}", report.version);
    let _ = writeln!(out, "platform: {}", report.platform);
    let _ = writeln!(out, "time:     {} (unix seconds)", report.unix_time);
    let _ = writeln!(out, "command:  {}", sanitize_single_line(report.command));
    let _ = writeln!(
        out,
        "where:    {}",
        without_home(report.location, report.home)
    );
    let _ = writeln!(out, "message:  {}", without_home(&message, report.home));
    out
}

/// Writes `text` as a new report in `dir` and prunes the oldest beyond [`KEEP`].
pub(crate) fn write(dir: &Path, unix_time: u64, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut path = dir.join(format!("crash-{unix_time}-{}.txt", std::process::id()));
    let mut attempt = 0;
    while path.exists() {
        attempt += 1;
        path = dir.join(format!(
            "crash-{unix_time}-{}-{attempt}.txt",
            std::process::id()
        ));
    }
    std::fs::write(&path, text)?;
    let mut all = list(dir);
    if all.len() > KEEP {
        all.sort();
        for old in all.iter().take(all.len() - KEEP) {
            let _ = std::fs::remove_file(dir.join(old));
        }
    }
    Ok(path)
}

/// The report file names in `dir`, oldest first.
pub(crate) fn list(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.starts_with("crash-")
                && Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
        })
        .collect();
    names.sort();
    names
}

/// Reads one report by file name. The name is looked up in the listing, never joined to the path
/// as given, so `../../etc/passwd` is not a report.
pub(crate) fn read(dir: &Path, name: &str) -> Option<String> {
    let known = list(dir).into_iter().find(|candidate| candidate == name)?;
    std::fs::read_to_string(dir.join(known)).ok()
}

/// The part of a panic location inside Klaub's own source tree: the builder's paths say nothing.
///
/// A build from a checkout reports `src/app/session.rs`; one from crates.io reports the path of
/// the unpacked `klaub-X.Y.Z` directory, which is cut off. Any other path (a dependency's) is
/// left as it is.
fn own_source(file: &str) -> &str {
    if file.starts_with("src/") {
        return file;
    }
    match file.rfind("/src/") {
        Some(at)
            if file[..at]
                .rsplit('/')
                .next()
                .is_some_and(|dir| dir.starts_with("klaub")) =>
        {
            &file[at + 1..]
        }
        _ => file,
    }
}

/// Installs the panic hook. The previous hook still runs, so the usual message and the terminal
/// restoration of the reader are unchanged.
pub(crate) fn install(dir: PathBuf, home: Option<PathBuf>, command: String) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".to_owned());
        let location = info.location().map_or_else(
            || "unknown".to_owned(),
            |l| format!("{}:{}", own_source(l.file()), l.line()),
        );
        let unix_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
        let text = render(&Report {
            version: env!("CARGO_PKG_VERSION"),
            platform: &platform,
            unix_time,
            command: &command,
            message: &message,
            location: &location,
            home: home.as_deref(),
        });
        // A failure to write a report must never hide the panic it describes.
        if write(&dir, unix_time, &text).is_ok() {
            crate::cli::output::error_line(
                "klaub crashed. A report was saved; `klaub crashes list` shows it. Nothing was sent.",
            );
        }
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample<'a>(message: &'a str, home: Option<&'a Path>) -> Report<'a> {
        Report {
            version: "0.1.0",
            platform: "linux x86_64",
            unix_time: 1_800_000_000,
            command: "review",
            message,
            location: "src/app/session.rs:10",
            home,
        }
    }

    #[test]
    fn a_location_is_cut_to_klaubs_own_source_tree() {
        assert_eq!(own_source("src/app/session.rs"), "src/app/session.rs");
        assert_eq!(
            own_source(
                "/home/ana/.cargo/registry/src/index.crates.io-1949/klaub-0.3.1/src/app/session.rs"
            ),
            "src/app/session.rs"
        );
        assert_eq!(
            own_source("/home/ana/build/klaub/src/tui/ui.rs"),
            "src/tui/ui.rs"
        );
        assert_eq!(
            own_source("/home/ana/.cargo/registry/src/index.crates.io-1949/serde-1.0.0/src/lib.rs"),
            "/home/ana/.cargo/registry/src/index.crates.io-1949/serde-1.0.0/src/lib.rs"
        );
    }

    #[test]
    fn a_report_names_the_failure_and_hides_the_home_directory() {
        let home = Path::new("/home/ana");
        let text = render(&sample(
            "could not read /home/ana/secret/notes.md",
            Some(home),
        ));
        assert!(text.contains("version:  0.1.0"));
        assert!(text.contains("command:  review"));
        assert!(text.contains("message:  could not read ~/secret/notes.md"));
        assert!(!text.contains("/home/ana"), "{text}");
    }

    #[test]
    fn the_message_is_one_clean_line_of_bounded_length() {
        let nasty = format!("first\u{1b}[31m line\nsecond {}", "x".repeat(2000));
        let text = render(&sample(&nasty, None));
        let message = text.lines().find(|l| l.starts_with("message:")).unwrap();
        assert!(message.len() < MAX_MESSAGE + 20, "{}", message.len());
        assert!(!text.contains('\u{1b}'));
        assert_eq!(text.lines().count(), 7, "one line per field");
    }

    #[test]
    fn reports_are_written_listed_read_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        for second in 0..(KEEP as u64 + 5) {
            write(dir.path(), 1_800_000_000 + second, "report\n").unwrap();
        }
        let names = list(dir.path());
        assert_eq!(names.len(), KEEP, "the oldest are removed");
        assert!(names[0].contains("1800000005"), "{names:?}");
        assert_eq!(read(dir.path(), &names[0]).as_deref(), Some("report\n"));
    }

    #[test]
    fn a_name_that_is_not_in_the_listing_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("secret.txt"), "private").unwrap();
        write(dir.path(), 1_800_000_000, "report\n").unwrap();
        assert_eq!(read(dir.path(), "secret.txt"), None);
        assert_eq!(read(dir.path(), "../secret.txt"), None);
        assert_eq!(read(dir.path(), "crash-nope.txt"), None);
    }
}
