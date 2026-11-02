//! `klaub feedback`: print the feedback prompt of a finished review.

use std::process::ExitCode;

use crate::adapters::storage::JsonReviewRepository;
use crate::domain::i18n::cli as t;
use crate::domain::ids::ReviewId;
use crate::domain::ports::ReviewRepository;
use crate::domain::review::{FeedbackRecord, ReviewStatus};
use anyhow::{Context as _, Result, bail};
use serde_json::json;

use crate::cli::args::{FeedbackArgs, Format};
use crate::cli::context::Context;
use crate::cli::output;

use super::worktree;

pub(super) fn run(ctx: &Context, args: &FeedbackArgs) -> Result<ExitCode> {
    let review_id = ReviewId::parse(&args.id).context(t::invalid_review_id(ctx.locale))?;
    let loaded = worktree::load(ctx)?;
    let review = JsonReviewRepository::new(&loaded.root)
        .load(&review_id)
        .with_context(|| t::cannot_load_review(ctx.locale, review_id.as_str()))?;
    let Some(feedback) = feedback_of(review.status, review.feedback) else {
        bail!("{}", t::review_not_finished(ctx.locale, review_id.as_str()));
    };
    match args.format {
        Format::Json => output::json(&json!({
            "review_id": review_id.as_str(),
            "prompt": feedback.prompt,
        }))?,
        Format::Text => output::line(&feedback.prompt),
    }
    Ok(ExitCode::SUCCESS)
}

/// The feedback of a review, once a round has actually been finished.
fn feedback_of(status: ReviewStatus, feedback: Option<FeedbackRecord>) -> Option<FeedbackRecord> {
    match status {
        ReviewStatus::Finished | ReviewStatus::Prepared | ReviewStatus::Submitted => feedback,
        ReviewStatus::Draft => None,
    }
}
