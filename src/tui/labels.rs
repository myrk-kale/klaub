//! Short words for enums that the reader shows. Every state has a word, so meaning never depends
//! on colour alone.
//!
//! The words come from the catalog in [`crate::domain::i18n::tui`]. The `label()` methods on the
//! domain types stay in English: exports and machine-readable output rely on them.

use crate::app::FileKind;
use crate::domain::config::Theme as ThemeName;
use crate::domain::finding::{QueueTag, ScoreComponent, Temporal};
use crate::domain::graph::ImpactScope;
use crate::domain::i18n::{Locale, tui as t};
use crate::domain::semantic::ChangeKind;

pub(crate) fn change_kind(locale: Locale, kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => t::change_added(locale),
        ChangeKind::Removed => t::change_removed(locale),
        ChangeKind::Modified => t::change_modified(locale),
        ChangeKind::Moved => t::change_moved(locale),
        ChangeKind::Renamed => t::change_renamed(locale),
        ChangeKind::SignatureChanged => t::change_signature(locale),
        ChangeKind::Rewritten => t::change_rewritten(locale),
        ChangeKind::FormatOnly => t::change_format(locale),
        ChangeKind::Unknown => t::change_unknown(locale),
    }
}

pub(crate) fn queue_tag(locale: Locale, tag: QueueTag) -> &'static str {
    match tag {
        QueueTag::Contract => t::tag_contract(locale),
        QueueTag::NewFile => t::tag_new(locale),
        QueueTag::RemovedFile => t::tag_removed(locale),
        QueueTag::Rewrite => t::tag_rewrite(locale),
        QueueTag::Moved => t::tag_moved(locale),
        QueueTag::FormatOnly => t::tag_format(locale),
        QueueTag::NoTests => t::tag_no_tests(locale),
        QueueTag::Unknown => t::tag_unknown(locale),
        QueueTag::Generated => t::tag_generated(locale),
    }
}

pub(crate) fn score_component(locale: Locale, component: ScoreComponent) -> &'static str {
    match component {
        ScoreComponent::Contract => t::component_contract(locale),
        ScoreComponent::Reach => t::component_reach(locale),
        ScoreComponent::Criticality => t::component_criticality(locale),
        ScoreComponent::Complexity => t::component_complexity(locale),
        ScoreComponent::TechnicalRisk => t::component_risk(locale),
        ScoreComponent::TestSignals => t::component_tests(locale),
        ScoreComponent::Architecture => t::component_architecture(locale),
        ScoreComponent::Mitigation => t::component_mitigation(locale),
    }
}

pub(crate) fn reach(locale: Locale, scope: ImpactScope) -> &'static str {
    match scope {
        ImpactScope::Local => t::reach_local(locale),
        ImpactScope::Module => t::reach_module(locale),
        ImpactScope::CrossModule => t::reach_cross_module(locale),
        ImpactScope::Global => t::reach_global(locale),
    }
}

pub(crate) fn temporal(locale: Locale, temporal: Temporal) -> &'static str {
    match temporal {
        Temporal::Introduced => t::temporal_introduced(locale),
        Temporal::Preexisting => t::temporal_preexisting(locale),
        Temporal::Removed => t::temporal_removed(locale),
        Temporal::Unknown => t::temporal_unknown(locale),
    }
}

pub(crate) fn file_kind(locale: Locale, kind: FileKind) -> &'static str {
    match kind {
        FileKind::Migration => t::file_kind_migration(locale),
        FileKind::DependencyManifest => t::file_kind_manifest(locale),
        FileKind::Config => t::file_kind_config(locale),
    }
}

/// The name of a colour theme. Most are proper names and stay as they are.
pub(crate) fn theme_title(locale: Locale, theme: ThemeName) -> &'static str {
    match theme {
        ThemeName::Light => t::theme_light(locale),
        ThemeName::HighContrast => t::theme_high_contrast(locale),
        other => other.title(),
    }
}
