//! Short words for enums that the reader shows. Every state has a word, so meaning never depends
//! on colour alone.
//!
//! The words come from the catalog in [`crate::domain::i18n::tui`]. The `label()` methods on the
//! domain types stay in English: exports and machine-readable output rely on them.

use crate::app::FileKind;
use crate::domain::config::Theme as ThemeName;
use crate::domain::finding::{QueueTag, ScoreComponent, Temporal};
use crate::domain::flow::FlowState;
use crate::domain::graph::{EdgeKind, EdgeOrigin, EntrypointKind, EntrypointScope, ImpactScope};
use crate::domain::i18n::{Locale, tui as t};
use crate::domain::review::{CommentStatus, CriterionStatus, FollowupStatus, ReviewStatus};
use crate::domain::semantic::ChangeKind;
use crate::domain::testing::{EvidenceStatus, LinkMethod, TestOutcome};

pub(crate) fn origin(locale: Locale, origin: EdgeOrigin) -> &'static str {
    match origin {
        EdgeOrigin::Resolved => t::origin_resolved(locale),
        EdgeOrigin::Heuristic => t::origin_heuristic(locale),
        EdgeOrigin::Unresolved => t::origin_unresolved(locale),
    }
}

/// A one-character certainty marker: solid for resolved, hollow for guesses.
pub(crate) fn origin_mark(origin: EdgeOrigin) -> char {
    match origin {
        EdgeOrigin::Resolved => '●',
        EdgeOrigin::Heuristic => '◐',
        EdgeOrigin::Unresolved => '○',
    }
}

pub(crate) fn entrypoint_kind(locale: Locale, kind: &EntrypointKind) -> String {
    match kind {
        EntrypointKind::Http { method, route } => {
            crate::domain::text::sanitize_single_line(&format!("{method} {route}"))
        }
        EntrypointKind::Cli => t::entrypoint_cli(locale).to_owned(),
        EntrypointKind::Main => t::entrypoint_main(locale).to_owned(),
        EntrypointKind::Worker => t::entrypoint_worker(locale).to_owned(),
        EntrypointKind::PublicExport => t::entrypoint_export(locale).to_owned(),
    }
}

pub(crate) fn entrypoint_scope(locale: Locale, scope: EntrypointScope) -> &'static str {
    match scope {
        EntrypointScope::External => t::entrypoint_external(locale),
        EntrypointScope::Internal => t::entrypoint_internal(locale),
    }
}

pub(crate) fn edge_kind(locale: Locale, kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Calls => t::edge_calls(locale),
        EdgeKind::Imports => t::edge_imports(locale),
        EdgeKind::Exports => t::edge_exports(locale),
        EdgeKind::Implements => t::edge_implements(locale),
        EdgeKind::Extends => t::edge_extends(locale),
        EdgeKind::References => t::edge_references(locale),
    }
}

pub(crate) fn comment_status(locale: Locale, status: CommentStatus) -> &'static str {
    match status {
        CommentStatus::Draft => t::comment_draft(locale),
        CommentStatus::Prepared => t::comment_prepared(locale),
        CommentStatus::Submitted => t::comment_submitted(locale),
        CommentStatus::Addressed => t::comment_addressed(locale),
        CommentStatus::Unresolved => t::comment_unresolved(locale),
        CommentStatus::Stale => t::comment_stale(locale),
    }
}

pub(crate) fn criterion_status(locale: Locale, status: CriterionStatus) -> &'static str {
    match status {
        CriterionStatus::NotChecked => t::criterion_not_checked(locale),
        CriterionStatus::Verified => t::criterion_verified(locale),
        CriterionStatus::Pending => t::criterion_pending(locale),
        CriterionStatus::NotApplicable => t::criterion_not_applicable(locale),
    }
}

pub(crate) fn criterion_mark(status: CriterionStatus) -> &'static str {
    match status {
        CriterionStatus::NotChecked => "[ ]",
        CriterionStatus::Verified => "[x]",
        CriterionStatus::Pending => "[~]",
        CriterionStatus::NotApplicable => "[-]",
    }
}

pub(crate) fn outcome(locale: Locale, outcome: TestOutcome) -> &'static str {
    match outcome {
        TestOutcome::Passed => t::outcome_passed(locale),
        TestOutcome::Failed => t::outcome_failed(locale),
        TestOutcome::Skipped => t::outcome_skipped(locale),
        TestOutcome::NotRun => t::outcome_not_run(locale),
    }
}

pub(crate) fn review_status(locale: Locale, status: ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Draft => t::review_draft(locale),
        ReviewStatus::Finished => t::review_finished(locale),
        ReviewStatus::Prepared => t::review_prepared(locale),
        ReviewStatus::Submitted => t::review_submitted(locale),
    }
}

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

pub(crate) fn evidence_status(locale: Locale, status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::NoAssociationFound => t::evidence_no_association(locale),
        EvidenceStatus::NotRun => t::evidence_not_run(locale),
        EvidenceStatus::Passed => t::evidence_passed(locale),
        EvidenceStatus::Failed => t::evidence_failed(locale),
        EvidenceStatus::Stale => t::evidence_stale(locale),
    }
}

pub(crate) fn link_method(locale: Locale, method: LinkMethod) -> &'static str {
    match method {
        LinkMethod::NameConvention => t::link_name(locale),
        LinkMethod::Import => t::link_import(locale),
        LinkMethod::Reference => t::link_reference(locale),
        LinkMethod::Configured => t::link_configured(locale),
    }
}

pub(crate) fn followup_status(locale: Locale, status: FollowupStatus) -> &'static str {
    match status {
        FollowupStatus::Pending => t::followup_pending(locale),
        FollowupStatus::Resolved => t::followup_resolved(locale),
        FollowupStatus::Partial => t::followup_partial(locale),
        FollowupStatus::Unresolved => t::followup_unresolved(locale),
        FollowupStatus::Regression => t::followup_regression(locale),
        FollowupStatus::Stale => t::followup_stale(locale),
    }
}

pub(crate) fn flow_state(locale: Locale, state: &FlowState) -> &'static str {
    match state {
        FlowState::Current => t::flow_current(locale),
        FlowState::Missing => t::flow_missing(locale),
        FlowState::Invalid { .. } => t::flow_invalid(locale),
        FlowState::Stale { .. } => t::flow_stale(locale),
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
