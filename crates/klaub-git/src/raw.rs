//! Parsing of `git diff --raw -z` output.

use klaub_core::{Error, Result};

/// One entry of `git diff --raw -z --no-abbrev`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawEntry {
    pub(crate) src_mode: String,
    pub(crate) dst_mode: String,
    pub(crate) src_sha: String,
    pub(crate) dst_sha: String,
    /// `A`, `M`, `D`, `R`, `C`, `T` or `U`.
    pub(crate) status: char,
    /// Similarity for renames and copies.
    pub(crate) score: Option<u8>,
    pub(crate) path: String,
    pub(crate) old_path: Option<String>,
}

impl RawEntry {
    /// The before side does not exist.
    pub(crate) fn before_absent(&self) -> bool {
        self.src_mode.chars().all(|c| c == '0')
    }

    /// The after side does not exist.
    pub(crate) fn after_absent(&self) -> bool {
        self.dst_mode.chars().all(|c| c == '0')
    }
}

/// Whether `sha` is the all-zero id git prints for content it did not hash.
pub(crate) fn is_zero_sha(sha: &str) -> bool {
    !sha.is_empty() && sha.chars().all(|c| c == '0')
}

/// Parses NUL-separated raw diff output.
///
/// Entries that are not valid UTF-8 are skipped (their paths cannot be represented) and counted.
pub(crate) fn parse_raw_z(output: &[u8]) -> Result<(Vec<RawEntry>, usize)> {
    let mut tokens = output
        .split(|byte| *byte == 0)
        .filter(|token| !token.is_empty());
    let mut entries = Vec::new();
    let mut skipped = 0;
    while let Some(meta) = tokens.next() {
        let meta = std::str::from_utf8(meta)
            .map_err(|_| Error::Git("unexpected bytes in git diff output".to_owned()))?;
        let Some(meta) = meta.strip_prefix(':') else {
            return Err(Error::Git(format!("unexpected git diff output: {meta:?}")));
        };
        let fields: Vec<&str> = meta.split_whitespace().collect();
        let [src_mode, dst_mode, src_sha, dst_sha, status] = fields.as_slice() else {
            return Err(Error::Git(format!("unexpected git diff entry: {meta:?}")));
        };
        let mut status_chars = status.chars();
        let letter = status_chars.next().unwrap_or('M');
        let score = status_chars.as_str().parse::<u8>().ok();
        let first = tokens
            .next()
            .ok_or_else(|| Error::Git("git diff output ended inside an entry".to_owned()))?;
        let second =
            if matches!(letter, 'R' | 'C') {
                Some(tokens.next().ok_or_else(|| {
                    Error::Git("git diff output ended inside a rename".to_owned())
                })?)
            } else {
                None
            };
        let to_string = |bytes: &[u8]| std::str::from_utf8(bytes).ok().map(str::to_owned);
        let (path, old_path) = match second {
            Some(new) => (to_string(new), to_string(first)),
            None => (to_string(first), None),
        };
        let Some(path) = path else {
            skipped += 1;
            continue;
        };
        entries.push(RawEntry {
            src_mode: (*src_mode).to_owned(),
            dst_mode: (*dst_mode).to_owned(),
            src_sha: (*src_sha).to_owned(),
            dst_sha: (*dst_sha).to_owned(),
            status: letter,
            score,
            path,
            old_path,
        });
    }
    Ok((entries, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO: &str = "0000000000000000000000000000000000000000";
    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";

    fn raw(entries: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for part in entries {
            out.extend_from_slice(part.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn parses_modified_added_deleted() {
        let output = raw(&[
            &format!(":100644 100644 {A} {B} M"),
            "src/a.py",
            &format!(":000000 100644 {ZERO} {B} A"),
            "src/new file.py",
            &format!(":100644 000000 {A} {ZERO} D"),
            "old.py",
        ]);
        let (entries, skipped) = parse_raw_z(&output).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(entries.len(), 3);
        assert_eq!(
            (entries[0].status, entries[0].path.as_str()),
            ('M', "src/a.py")
        );
        assert!(entries[1].before_absent() && !entries[1].after_absent());
        assert_eq!(entries[1].path, "src/new file.py");
        assert!(entries[2].after_absent());
        assert!(is_zero_sha(&entries[2].dst_sha));
    }

    #[test]
    fn parses_renames_with_both_paths_and_a_score() {
        let output = raw(&[
            &format!(":100644 100644 {A} {B} R087"),
            "old/name.py",
            "new/name.py",
        ]);
        let (entries, _) = parse_raw_z(&output).unwrap();
        assert_eq!(entries[0].status, 'R');
        assert_eq!(entries[0].score, Some(87));
        assert_eq!(entries[0].old_path.as_deref(), Some("old/name.py"));
        assert_eq!(entries[0].path, "new/name.py");
    }

    #[test]
    fn skips_paths_that_are_not_utf8() {
        let mut output = format!(":100644 100644 {A} {B} M\0").into_bytes();
        output.extend_from_slice(b"bad\xff.py\0");
        let (entries, skipped) = parse_raw_z(&output).unwrap();
        assert_eq!(entries, Vec::new());
        assert_eq!(skipped, 1);
    }

    #[test]
    fn rejects_garbage_and_truncated_output() {
        assert!(parse_raw_z(b"garbage\0").is_err());
        assert!(parse_raw_z(format!(":100644 100644 {A} {B} M\0").as_bytes()).is_err());
        assert_eq!(parse_raw_z(b"").unwrap().0, Vec::new());
    }

    #[test]
    fn zero_sha_detection() {
        assert!(is_zero_sha(ZERO));
        assert!(!is_zero_sha(A));
        assert!(!is_zero_sha(""));
    }
}
