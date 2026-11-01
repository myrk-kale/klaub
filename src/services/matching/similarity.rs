//! Similarity measures between symbols.

use crate::domain::symbol::{Symbol, SymbolKind};
use similar::TextDiff;

/// Symbols with fewer tokens than this are too small to match by similarity alone.
pub(crate) const MIN_FUZZY_TOKENS: usize = 6;

/// Weights of the match score. They sum to 1.0 and are versioned: change them only with
/// fixture evidence.
pub(crate) const W_TOKENS: f32 = 0.45;
pub(crate) const W_SHAPE: f32 = 0.25;
pub(crate) const W_SIGNATURE: f32 = 0.10;
pub(crate) const W_NAME: f32 = 0.10;
pub(crate) const W_LOCATION: f32 = 0.10;

/// Kinds that may be the same entity after a change. `None` means "never fuzzy-matched".
pub(crate) fn family(kind: SymbolKind) -> Option<u8> {
    match kind {
        SymbolKind::Function | SymbolKind::Method => Some(1),
        SymbolKind::Class
        | SymbolKind::Struct
        | SymbolKind::Interface
        | SymbolKind::Enum
        | SymbolKind::Trait
        | SymbolKind::TypeAlias => Some(2),
        SymbolKind::Impl => Some(3),
        SymbolKind::Constant => Some(4),
        SymbolKind::Module => None,
    }
}

/// Multiset Jaccard similarity of two token sequences, in `0.0..=1.0`.
pub(crate) fn token_similarity(a: &[u32], b: &[u32]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut left = a.to_vec();
    let mut right = b.to_vec();
    left.sort_unstable();
    right.sort_unstable();
    let (mut i, mut j) = (0, 0);
    let (mut shared, mut total) = (0_usize, 0_usize);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Equal => {
                shared += 1;
                total += 1;
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                total += 1;
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                total += 1;
                j += 1;
            }
        }
    }
    total += (left.len() - i) + (right.len() - j);
    #[allow(clippy::cast_precision_loss)]
    {
        shared as f32 / total as f32
    }
}

/// Similarity of two simple names, in `0.0..=1.0`.
pub(crate) fn name_similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    TextDiff::from_chars(a, b).ratio()
}

/// Similarity of the declarations once the symbol's own name is masked out.
pub(crate) fn signature_similarity(before: &Symbol, after: &Symbol) -> f32 {
    let mask = |s: &Symbol| s.signature.replacen(&s.name, "_", 1);
    let (a, b) = (mask(before), mask(after));
    if a == b {
        return 1.0;
    }
    TextDiff::from_chars(a.as_str(), b.as_str()).ratio()
}

/// The part of a qualified name before its last segment (`A.B.f` -> `A.B`).
pub(crate) fn container_of(qualified: &str) -> &str {
    qualified.rsplit_once('.').map_or("", |(parent, _)| parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_similarity_is_a_multiset_jaccard() {
        assert!((token_similarity(&[1, 2, 3], &[1, 2, 3]) - 1.0).abs() < f32::EPSILON);
        assert!((token_similarity(&[1, 2, 3], &[4, 5, 6])).abs() < f32::EPSILON);
        assert!(
            (token_similarity(&[1, 1, 2], &[1, 2, 2]) - 0.5).abs() < f32::EPSILON,
            "min/max of counts: (1+1)/(2+2)"
        );
        assert!((token_similarity(&[], &[]) - 1.0).abs() < f32::EPSILON);
        assert!(token_similarity(&[], &[1]).abs() < f32::EPSILON);
    }

    #[test]
    fn name_similarity_rewards_shared_stems() {
        assert!(name_similarity("validate_token", "validate_tokens") > 0.9);
        assert!(name_similarity("validate", "render") < 0.5);
        assert!((name_similarity("x", "x") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn containers() {
        assert_eq!(container_of("A.B.f"), "A.B");
        assert_eq!(container_of("f"), "");
    }

    #[test]
    fn weights_sum_to_one() {
        let sum = W_TOKENS + W_SHAPE + W_SIGNATURE + W_NAME + W_LOCATION;
        assert!((sum - 1.0).abs() < 1e-6);
    }
}
