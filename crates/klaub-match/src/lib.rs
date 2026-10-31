//! # klaub-match
//!
//! A pure domain service: given the symbols of two versions of the same files, decide which
//! symbol is which and how it changed.
//!
//! Matching goes from certain to plausible (ADR 0007):
//!
//! 1. **Identity.** Same kind and qualified name. The comparison then looks at the text, the
//!    tokens, the declaration and the amount of body that survived, and yields `Modified`,
//!    `SignatureChanged`, `Rewritten` or `FormatOnly`, with confidence 1.0.
//! 2. **Similarity within a file** for what is left: a weighted score of token similarity,
//!    syntax shape, declaration, name and location finds renames and moves.
//! 3. **Across files**, the same score plus unique qualified names finds symbols that moved.
//!
//! A score at or above the strong threshold (0.85) is a firm match, between the probable
//! threshold (0.65) and strong it is shown as *probable*, and below it nothing is claimed:
//! the symbol stays `Added`/`Removed` and, when a near miss exists, the evidence says so.

mod linediff;
mod similarity;

use std::collections::{HashMap, HashSet, VecDeque};

use klaub_core::path::RepoPath;
use klaub_core::semantic::{
    ChangeKind, Confidence, Hunk, LineDiff, MatchThresholds, SymbolChange, SymbolRef,
    symbol_unit_id,
};
use klaub_core::symbol::{ParsedFile, Span, Symbol, SymbolKind};

pub use linediff::line_diff;
use similarity::{
    MIN_FUZZY_TOKENS, W_LOCATION, W_NAME, W_SHAPE, W_SIGNATURE, W_TOKENS, container_of, family,
    name_similarity, signature_similarity, token_similarity,
};

/// Below this token similarity an unchanged identity counts as a rewrite.
const REWRITE_BELOW: f32 = 0.35;
/// Candidates that scored at least this much but not enough are mentioned in the evidence.
const NEAR_MISS: f32 = 0.40;
/// Above this many candidate pairs the exhaustive comparison is replaced by a hash join.
const MAX_EXHAUSTIVE_PAIRS: usize = 250_000;
/// Confidence given to a unique qualified-name match in another file.
const SAME_NAME_ELSEWHERE: f32 = 0.9;

/// One changed file, with whichever sides exist.
#[derive(Debug, Clone, Copy)]
pub struct FilePair<'a> {
    /// Path after the change (or before, for deleted files).
    pub path: &'a RepoPath,
    /// Path before the change when the file was renamed.
    pub old_path: Option<&'a RepoPath>,
    /// Parse of the previous version.
    pub before: Option<&'a ParsedFile>,
    /// Parse of the new version.
    pub after: Option<&'a ParsedFile>,
}

/// A symbol of one side of one file pair.
#[derive(Debug, Clone, Copy)]
struct Handle<'a> {
    pair: usize,
    index: usize,
    symbol: &'a Symbol,
}

type Key = (usize, usize);

impl Handle<'_> {
    fn key(self) -> Key {
        (self.pair, self.index)
    }
}

struct Ctx<'a> {
    pairs: &'a [FilePair<'a>],
}

impl<'a> Ctx<'a> {
    fn before_path(&self, h: Handle<'_>) -> &'a RepoPath {
        let pair = &self.pairs[h.pair];
        pair.old_path.unwrap_or(pair.path)
    }

    fn after_path(&self, h: Handle<'_>) -> &'a RepoPath {
        self.pairs[h.pair].path
    }

    fn before_ref(&self, h: Handle<'_>) -> SymbolRef {
        symbol_ref(self.before_path(h), h.index, h.symbol)
    }

    fn after_ref(&self, h: Handle<'_>) -> SymbolRef {
        symbol_ref(self.after_path(h), h.index, h.symbol)
    }
}

fn symbol_ref(file: &RepoPath, index: usize, symbol: &Symbol) -> SymbolRef {
    SymbolRef {
        file: file.clone(),
        index,
        qualified_name: symbol.qualified_name.clone(),
        kind: symbol.kind,
        signature: symbol.signature.clone(),
        span: symbol.span,
    }
}

/// Which sides a change has.
enum Sides {
    Both(SymbolRef, SymbolRef),
    Added(SymbolRef),
    Removed(SymbolRef),
}

fn make_change(
    kind: ChangeKind,
    sides: Sides,
    confidence: f32,
    evidence: Vec<String>,
) -> SymbolChange {
    let id_of = |r: &SymbolRef| symbol_unit_id(&r.file, &r.qualified_name, r.kind);
    let (id, before, after) = match sides {
        Sides::Both(b, a) => (id_of(&a), Some(b), Some(a)),
        Sides::Added(a) => (id_of(&a), None, Some(a)),
        Sides::Removed(b) => (id_of(&b), Some(b), None),
    };
    SymbolChange {
        id,
        kind,
        before,
        after,
        confidence: Confidence::new(confidence),
        evidence,
    }
}

/// How an identity match changed, or `None` if the symbol is untouched.
fn classify_identity(before: &Symbol, after: &Symbol) -> Option<(ChangeKind, Vec<String>)> {
    if before.fingerprint.text_hash == after.fingerprint.text_hash
        && before.signature == after.signature
    {
        return None;
    }
    if before.fingerprint.token_hash == after.fingerprint.token_hash {
        return Some((
            ChangeKind::FormatOnly,
            vec!["only whitespace or comments changed".to_owned()],
        ));
    }
    let similarity = token_similarity(&before.fingerprint.tokens, &after.fingerprint.tokens);
    let mut evidence = vec!["same kind and qualified name".to_owned()];
    let declaration_changed = before.signature != after.signature
        || before.decorators != after.decorators
        || before.visibility != after.visibility;
    if declaration_changed {
        if before.signature != after.signature {
            evidence.push(format!(
                "declaration changed: `{}` → `{}`",
                before.signature, after.signature
            ));
        }
        if before.visibility != after.visibility {
            evidence.push(format!(
                "visibility changed: {:?} → {:?}",
                before.visibility, after.visibility
            ));
        }
        if before.decorators != after.decorators {
            evidence.push("decorators changed".to_owned());
        }
        if similarity < 1.0 {
            evidence.push(format!(
                "body changed too ({:.0}% of the tokens survive)",
                similarity * 100.0
            ));
        }
        return Some((ChangeKind::SignatureChanged, evidence));
    }
    let enough_tokens = before.fingerprint.tokens.len() >= MIN_FUZZY_TOKENS
        && after.fingerprint.tokens.len() >= MIN_FUZZY_TOKENS;
    if enough_tokens && similarity < REWRITE_BELOW {
        evidence.push(format!(
            "only {:.0}% of the tokens survive",
            similarity * 100.0
        ));
        return Some((ChangeKind::Rewritten, evidence));
    }
    evidence.push(format!("{:.0}% of the tokens survive", similarity * 100.0));
    Some((ChangeKind::Modified, evidence))
}

#[derive(Debug, Clone, Copy)]
struct Score {
    total: f32,
    tokens: f32,
    shape: bool,
    name: f32,
    signature: f32,
    location: f32,
}

fn score(ctx: &Ctx<'_>, before: Handle<'_>, after: Handle<'_>) -> Score {
    let (sb, sa) = (before.symbol, after.symbol);
    let tokens = token_similarity(&sb.fingerprint.tokens, &sa.fingerprint.tokens);
    let shape = sb.fingerprint.shape_hash == sa.fingerprint.shape_hash;
    let signature = signature_similarity(sb, sa);
    let name = name_similarity(&sb.name, &sa.name);
    let same_file = ctx.before_path(before) == ctx.after_path(after);
    let same_container = container_of(&sb.qualified_name) == container_of(&sa.qualified_name);
    let location = match (same_file, same_container) {
        (true, true) => 1.0,
        (true, false) => 0.5,
        (false, true) => 0.3,
        (false, false) => 0.0,
    };
    let total = W_TOKENS * tokens
        + W_SHAPE * f32::from(u8::from(shape))
        + W_SIGNATURE * signature
        + W_NAME * name
        + W_LOCATION * location;
    Score {
        total,
        tokens,
        shape,
        name,
        signature,
        location,
    }
}

fn comparable(before: Handle<'_>, after: Handle<'_>) -> bool {
    let (sb, sa) = (before.symbol, after.symbol);
    let (Some(fb), Some(fa)) = (family(sb.kind), family(sa.kind)) else {
        return false;
    };
    let (nb, na) = (sb.fingerprint.tokens.len(), sa.fingerprint.tokens.len());
    if fb != fa || nb < MIN_FUZZY_TOKENS || na < MIN_FUZZY_TOKENS {
        return false;
    }
    let (small, large) = if nb < na { (nb, na) } else { (na, nb) };
    small * 5 >= large * 2 // within a factor of 2.5
}

struct Found<'a> {
    before: Handle<'a>,
    after: Handle<'a>,
    score: Score,
}

/// Best rejected candidate per symbol: `(is_before_side, key) -> (score, other's name)`.
type NearMisses = HashMap<(bool, Key), (f32, String)>;

fn remember(near: &mut NearMisses, key: (bool, Key), note: (f32, String)) {
    if near.get(&key).is_none_or(|(best, _)| note.0 > *best) {
        near.insert(key, note);
    }
}

type Leftovers<'a> = Vec<Handle<'a>>;

/// Pairs `before` with `after` greedily by descending score.
fn fuzzy<'a>(
    ctx: &Ctx<'a>,
    before: &[Handle<'a>],
    after: &[Handle<'a>],
    thresholds: MatchThresholds,
    near: &mut NearMisses,
) -> (Vec<Found<'a>>, Leftovers<'a>, Leftovers<'a>) {
    let mut candidates: Vec<Found<'a>> = Vec::new();
    let mut consider = |b: Handle<'a>, a: Handle<'a>, near: &mut NearMisses| {
        if !comparable(b, a) {
            return;
        }
        let s = score(ctx, b, a);
        if s.total >= thresholds.probable {
            candidates.push(Found {
                before: b,
                after: a,
                score: s,
            });
        } else if s.total >= NEAR_MISS {
            remember(
                near,
                (true, b.key()),
                (s.total, a.symbol.qualified_name.clone()),
            );
            remember(
                near,
                (false, a.key()),
                (s.total, b.symbol.qualified_name.clone()),
            );
        }
    };

    if before.len().saturating_mul(after.len()) <= MAX_EXHAUSTIVE_PAIRS {
        for &b in before {
            for &a in after {
                consider(b, a, near);
            }
        }
    } else {
        // Too many pairs to compare exhaustively: only symbols with the same syntax shape.
        let mut by_shape: HashMap<u64, Vec<Handle<'a>>> = HashMap::new();
        for &a in after {
            by_shape
                .entry(a.symbol.fingerprint.shape_hash)
                .or_default()
                .push(a);
        }
        for &b in before {
            for &a in by_shape
                .get(&b.symbol.fingerprint.shape_hash)
                .into_iter()
                .flatten()
            {
                consider(b, a, near);
            }
        }
    }

    candidates.sort_by(|x, y| {
        y.score
            .total
            .total_cmp(&x.score.total)
            .then(x.before.key().cmp(&y.before.key()))
            .then(x.after.key().cmp(&y.after.key()))
    });
    let mut used_before: HashSet<Key> = HashSet::new();
    let mut used_after: HashSet<Key> = HashSet::new();
    let mut matched = Vec::new();
    for candidate in candidates {
        if used_before.contains(&candidate.before.key())
            || used_after.contains(&candidate.after.key())
        {
            continue;
        }
        used_before.insert(candidate.before.key());
        used_after.insert(candidate.after.key());
        matched.push(candidate);
    }
    let rest_before = before
        .iter()
        .copied()
        .filter(|h| !used_before.contains(&h.key()))
        .collect();
    let rest_after = after
        .iter()
        .copied()
        .filter(|h| !used_after.contains(&h.key()))
        .collect();
    (matched, rest_before, rest_after)
}

fn container_or_root(qualified: &str) -> &str {
    let container = container_of(qualified);
    if container.is_empty() {
        "<module>"
    } else {
        container
    }
}

fn describe_match(ctx: &Ctx<'_>, found: &Found<'_>) -> SymbolChange {
    let (sb, sa) = (found.before.symbol, found.after.symbol);
    let (path_b, path_a) = (ctx.before_path(found.before), ctx.after_path(found.after));
    let renamed = sb.name != sa.name;
    let moved =
        path_b != path_a || container_of(&sb.qualified_name) != container_of(&sa.qualified_name);
    let kind = if moved {
        ChangeKind::Moved
    } else if renamed {
        ChangeKind::Renamed
    } else {
        ChangeKind::Modified
    };
    let mut evidence = Vec::new();
    if renamed {
        evidence.push(format!(
            "renamed from `{}` to `{}`",
            sb.qualified_name, sa.qualified_name
        ));
    }
    if moved {
        if path_b == path_a {
            evidence.push(format!(
                "moved within {path_a}: `{}` → `{}`",
                container_or_root(&sb.qualified_name),
                container_or_root(&sa.qualified_name)
            ));
        } else {
            evidence.push(format!("moved from {path_b} to {path_a}"));
        }
    }
    if sb.fingerprint.token_hash == sa.fingerprint.token_hash {
        evidence.push("the code itself is unchanged".to_owned());
    } else {
        evidence.push(format!(
            "{:.0}% of the tokens survive",
            found.score.tokens * 100.0
        ));
    }
    evidence.push(format!(
        "score {:.2} (tokens {:.2}, shape {}, declaration {:.2}, name {:.2}, location {:.2})",
        found.score.total,
        found.score.tokens,
        if found.score.shape {
            "equal"
        } else {
            "different"
        },
        found.score.signature,
        found.score.name,
        found.score.location
    ));
    make_change(
        kind,
        Sides::Both(ctx.before_ref(found.before), ctx.after_ref(found.after)),
        found.score.total,
        evidence,
    )
}

fn handles(pair: usize, file: Option<&ParsedFile>) -> Vec<Handle<'_>> {
    file.map(|f| {
        f.symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| Handle {
                pair,
                index,
                symbol,
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Matches the symbols of every changed file and classifies each change.
///
/// The result is sorted by file and position. Untouched symbols produce no entry.
pub fn match_symbols(pairs: &[FilePair<'_>], thresholds: MatchThresholds) -> Vec<SymbolChange> {
    let ctx = Ctx { pairs };
    let mut changes: Vec<SymbolChange> = Vec::new();
    let mut near = NearMisses::new();
    let mut leftovers: Vec<(Leftovers<'_>, Leftovers<'_>)> = Vec::with_capacity(pairs.len());

    // 1. Identity, per file.
    for (p, pair) in pairs.iter().enumerate() {
        let before_handles = handles(p, pair.before);
        let mut by_identity: HashMap<(SymbolKind, &str), VecDeque<Handle<'_>>> = HashMap::new();
        for h in &before_handles {
            by_identity
                .entry((h.symbol.kind, h.symbol.qualified_name.as_str()))
                .or_default()
                .push_back(*h);
        }
        let mut matched_before: HashSet<usize> = HashSet::new();
        let mut rest_after = Vec::new();
        for ah in handles(p, pair.after) {
            let found = by_identity
                .get_mut(&(ah.symbol.kind, ah.symbol.qualified_name.as_str()))
                .and_then(VecDeque::pop_front);
            let Some(bh) = found else {
                rest_after.push(ah);
                continue;
            };
            matched_before.insert(bh.index);
            if let Some((kind, evidence)) = classify_identity(bh.symbol, ah.symbol) {
                // Whitespace between top-level definitions is not worth a unit.
                let noise = ah.symbol.kind == SymbolKind::Module && kind == ChangeKind::FormatOnly;
                if !noise {
                    changes.push(make_change(
                        kind,
                        Sides::Both(ctx.before_ref(bh), ctx.after_ref(ah)),
                        1.0,
                        evidence,
                    ));
                }
            }
        }
        let rest_before: Vec<Handle<'_>> = before_handles
            .into_iter()
            .filter(|h| !matched_before.contains(&h.index))
            .collect();
        leftovers.push((rest_before, rest_after));
    }

    // 2. Similarity within each file (renames, moves between classes).
    for (rest_before, rest_after) in &mut leftovers {
        if rest_before.is_empty() || rest_after.is_empty() {
            continue;
        }
        let (matched, left_before, left_after) =
            fuzzy(&ctx, rest_before, rest_after, thresholds, &mut near);
        changes.extend(matched.iter().map(|m| describe_match(&ctx, m)));
        *rest_before = left_before;
        *rest_after = left_after;
    }

    // 3. Across files.
    let all_before: Vec<Handle<'_>> = leftovers
        .iter()
        .flat_map(|(b, _)| b.iter().copied())
        .collect();
    let all_after: Vec<Handle<'_>> = leftovers
        .iter()
        .flat_map(|(_, a)| a.iter().copied())
        .collect();
    let (mut rest_before, mut rest_after) = if all_before.is_empty() || all_after.is_empty() {
        (all_before, all_after)
    } else {
        let (matched, rest_before, rest_after) =
            fuzzy(&ctx, &all_before, &all_after, thresholds, &mut near);
        changes.extend(matched.iter().map(|m| describe_match(&ctx, m)));
        (rest_before, rest_after)
    };

    // 3b. A qualified name that is unique on both sides is the same symbol in another file.
    let mut by_name_before: HashMap<(SymbolKind, &str), Vec<Handle<'_>>> = HashMap::new();
    let mut by_name_after: HashMap<(SymbolKind, &str), Vec<Handle<'_>>> = HashMap::new();
    for &h in rest_before
        .iter()
        .filter(|h| h.symbol.kind != SymbolKind::Module)
    {
        by_name_before
            .entry((h.symbol.kind, h.symbol.qualified_name.as_str()))
            .or_default()
            .push(h);
    }
    for &h in rest_after
        .iter()
        .filter(|h| h.symbol.kind != SymbolKind::Module)
    {
        by_name_after
            .entry((h.symbol.kind, h.symbol.qualified_name.as_str()))
            .or_default()
            .push(h);
    }
    let mut paired: HashSet<(bool, Key)> = HashSet::new();
    for (key, before) in &by_name_before {
        if let (&[b], Some(&[a])) = (before.as_slice(), by_name_after.get(key).map(Vec::as_slice))
            && ctx.before_path(b) != ctx.after_path(a)
        {
            paired.insert((true, b.key()));
            paired.insert((false, a.key()));
            let evidence = vec![
                format!("moved from {} to {}", ctx.before_path(b), ctx.after_path(a)),
                "same kind and qualified name in another file".to_owned(),
            ];
            changes.push(make_change(
                ChangeKind::Moved,
                Sides::Both(ctx.before_ref(b), ctx.after_ref(a)),
                SAME_NAME_ELSEWHERE,
                evidence,
            ));
        }
    }
    rest_before.retain(|h| !paired.contains(&(true, h.key())));
    rest_after.retain(|h| !paired.contains(&(false, h.key())));

    // 4. What is left was removed or added.
    let near_note = |before_side: bool, h: Handle<'_>| -> Option<String> {
        near.get(&(before_side, h.key())).map(|(best, name)| {
            format!(
                "closest candidate `{name}` scored {best:.2}, below the {:.2} needed to call it a move",
                thresholds.probable
            )
        })
    };
    for h in rest_before {
        if h.symbol.kind == SymbolKind::Module && h.symbol.fingerprint.tokens.is_empty() {
            continue;
        }
        let mut evidence = vec!["no counterpart in the new version".to_owned()];
        evidence.extend(near_note(true, h));
        changes.push(make_change(
            ChangeKind::Removed,
            Sides::Removed(ctx.before_ref(h)),
            1.0,
            evidence,
        ));
    }
    for h in rest_after {
        if h.symbol.kind == SymbolKind::Module && h.symbol.fingerprint.tokens.is_empty() {
            continue;
        }
        let mut evidence = vec!["no counterpart in the previous version".to_owned()];
        evidence.extend(near_note(false, h));
        changes.push(make_change(
            ChangeKind::Added,
            Sides::Added(ctx.after_ref(h)),
            1.0,
            evidence,
        ));
    }

    changes.sort_by_cached_key(|change| {
        change
            .after
            .as_ref()
            .or(change.before.as_ref())
            .map(|r| (r.file.clone(), r.span.start_line, r.qualified_name.clone()))
    });
    changes
}

/// The hunks of `diff` that touch `span`, on the after side (`after = true`) or the before side.
pub fn hunks_touching<'a>(diff: &'a LineDiff, span: &Span, after: bool) -> Vec<&'a Hunk> {
    diff.hunks
        .iter()
        .filter(|hunk| {
            let range = if after { hunk.after } else { hunk.before };
            range.is_some_and(|r| r.start <= span.end_line && r.end >= span.start_line)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use klaub_core::semantic::{LineRange, MatchBand};
    use klaub_core::symbol::{Fingerprint, SymbolFeatures, Visibility};

    fn sym(name: &str, qualified: &str, kind: SymbolKind, tokens: &[u32], shape: u64) -> Symbol {
        let sum: u64 = tokens.iter().map(|t| u64::from(*t)).sum();
        Symbol {
            name: name.to_owned(),
            qualified_name: qualified.to_owned(),
            kind,
            signature: format!("def {name}()"),
            visibility: Visibility::Public,
            span: Span {
                start_byte: 0,
                end_byte: 0,
                start_line: 1,
                end_line: 3,
            },
            parent: Some(0),
            decorators: vec![],
            fingerprint: Fingerprint {
                shape_hash: shape,
                token_hash: sum,
                text_hash: sum + 1,
                tokens: tokens.to_vec(),
            },
            features: SymbolFeatures::default(),
        }
    }

    fn file(symbols: Vec<Symbol>) -> ParsedFile {
        let mut all = vec![sym("<module>", "<module>", SymbolKind::Module, &[], 0)];
        all.extend(symbols);
        ParsedFile {
            language: klaub_core::language::Language::Python,
            symbols: all,
            imports: vec![],
            calls: vec![],
            entrypoints: vec![],
            has_syntax_errors: false,
            parser_version: 1,
        }
    }

    fn path(p: &str) -> RepoPath {
        RepoPath::new(p).unwrap()
    }

    fn run(before: Option<&ParsedFile>, after: Option<&ParsedFile>) -> Vec<SymbolChange> {
        let p = path("a.py");
        match_symbols(
            &[FilePair {
                path: &p,
                old_path: None,
                before,
                after,
            }],
            MatchThresholds::default(),
        )
    }

    #[test]
    fn identical_symbols_produce_no_change() {
        let f = file(vec![sym(
            "f",
            "f",
            SymbolKind::Function,
            &[1, 2, 3, 4, 5, 6, 7],
            9,
        )]);
        assert!(run(Some(&f), Some(&f)).is_empty());
    }

    #[test]
    fn added_and_removed_files() {
        let f = file(vec![sym(
            "f",
            "f",
            SymbolKind::Function,
            &[1, 2, 3, 4, 5, 6, 7],
            9,
        )]);
        let added = run(None, Some(&f));
        assert_eq!(
            added.iter().map(|c| c.kind).collect::<Vec<_>>(),
            vec![ChangeKind::Added]
        );
        assert!(added[0].before.is_none() && added[0].after.is_some());
        let removed = run(Some(&f), None);
        assert_eq!(
            removed.iter().map(|c| c.kind).collect::<Vec<_>>(),
            vec![ChangeKind::Removed]
        );
    }

    #[test]
    fn a_rename_with_the_same_body_is_found_with_confidence() {
        let tokens = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19];
        let before = file(vec![sym(
            "validate_token",
            "validate_token",
            SymbolKind::Function,
            &tokens,
            5,
        )]);
        let after = file(vec![sym(
            "check_token",
            "check_token",
            SymbolKind::Function,
            &tokens,
            5,
        )]);
        let changes = run(Some(&before), Some(&after));
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(changes[0].kind, ChangeKind::Renamed);
        assert_ne!(
            changes[0].confidence.band(MatchThresholds::default()),
            MatchBand::Unresolved
        );
        assert!(
            changes[0]
                .evidence
                .iter()
                .any(|e| e.contains("renamed from `validate_token` to `check_token`"))
        );
    }

    #[test]
    fn unrelated_symbols_are_not_called_a_move() {
        let before = file(vec![sym(
            "alpha",
            "alpha",
            SymbolKind::Function,
            &[1, 2, 3, 4, 5, 6, 7, 8],
            1,
        )]);
        let after = file(vec![sym(
            "omega",
            "omega",
            SymbolKind::Function,
            &[91, 92, 93, 94, 95, 96, 97, 98],
            2,
        )]);
        let kinds: Vec<_> = run(Some(&before), Some(&after))
            .iter()
            .map(|c| c.kind)
            .collect();
        assert_eq!(kinds.len(), 2);
        assert!(kinds.contains(&ChangeKind::Added) && kinds.contains(&ChangeKind::Removed));
    }

    #[test]
    fn hunks_touching_a_span() {
        let diff = LineDiff {
            before: vec![],
            after: vec![],
            hunks: vec![
                Hunk {
                    before: None,
                    after: Some(LineRange { start: 2, end: 3 }),
                },
                Hunk {
                    before: Some(LineRange { start: 10, end: 11 }),
                    after: None,
                },
            ],
        };
        let span = Span {
            start_byte: 0,
            end_byte: 0,
            start_line: 1,
            end_line: 5,
        };
        assert_eq!(hunks_touching(&diff, &span, true).len(), 1);
        assert_eq!(hunks_touching(&diff, &span, false).len(), 0);
    }
}
