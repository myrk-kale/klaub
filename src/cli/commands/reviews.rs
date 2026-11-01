//! `klaub reviews`: list, show and export the reviews stored in this worktree.

use std::process::ExitCode;

use crate::adapters::storage::JsonReviewRepository;
use crate::domain::ids::ReviewId;
use crate::domain::ports::ReviewRepository;
use crate::domain::review::Review;
use crate::services::review::{export_markdown, export_sarif};
use anyhow::{Context as _, Result};
use serde_json::json;

use crate::cli::args::{ExportFormat, Format, ReviewsCommand};
use crate::cli::context::Context;
use crate::cli::output;

use super::worktree;

pub(super) fn run(ctx: &Context, command: ReviewsCommand) -> Result<ExitCode> {
    match command {
        ReviewsCommand::List(args) => list(ctx, args.format),
        ReviewsCommand::Show { id, format } => show(ctx, &id, format),
        ReviewsCommand::Export { id, format } => export(ctx, &id, format),
    }
}

fn repository(ctx: &Context) -> Result<(worktree::Worktree, JsonReviewRepository)> {
    let loaded = worktree::load(ctx)?;
    let repository = JsonReviewRepository::new(&loaded.root);
    Ok((loaded, repository))
}

fn list(ctx: &Context, format: Format) -> Result<ExitCode> {
    let (_, repository) = repository(ctx)?;
    let reviews = repository.list()?;
    match format {
        Format::Json => {
            let rows: Vec<_> = reviews
                .iter()
                .map(|review| {
                    json!({
                        "id": review.review_id.as_str(),
                        "status": review.status,
                        "comments": review.comment_count,
                        "updated": review.updated_at.to_string(),
                        "snapshot_id": review.snapshot_id.as_str(),
                    })
                })
                .collect();
            output::json(&json!(rows))?;
        }
        Format::Text => {
            for review in &reviews {
                output::line(&format!(
                    "{}  {:<9}  {:>4} comments  updated {}",
                    review.review_id,
                    status_label(review.status),
                    review.comment_count,
                    review.updated_at
                ));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn show(ctx: &Context, id: &str, format: Format) -> Result<ExitCode> {
    let review = load_review(ctx, id)?;
    match format {
        Format::Json => print_document(&review)?,
        Format::Text => print_summary(&review),
    }
    Ok(ExitCode::SUCCESS)
}

fn export(ctx: &Context, id: &str, format: ExportFormat) -> Result<ExitCode> {
    let review = load_review(ctx, id)?;
    match format {
        ExportFormat::Json => print_document(&review)?,
        ExportFormat::Markdown => output::line(export_markdown(&review).trim_end()),
        ExportFormat::Sarif => output::json(&export_sarif(&review))?,
    }
    Ok(ExitCode::SUCCESS)
}

fn load_review(ctx: &Context, id: &str) -> Result<Review> {
    let review_id = ReviewId::parse(id).context("invalid review id")?;
    let (_, repository) = repository(ctx)?;
    repository
        .load(&review_id)
        .with_context(|| format!("cannot load review {review_id}"))
}

fn print_document(review: &Review) -> Result<()> {
    let document = serde_json::to_value(review)?;
    output::json(&document)
}

fn print_summary(review: &Review) {
    output::line(&format!(
        "{}  {}  {} comments",
        review.review_id,
        status_label(review.status),
        review.comments.len()
    ));
    for comment in &review.comments {
        output::line(&format!(
            "  {}  {}: {}",
            comment.comment_id,
            comment.anchor.file_path,
            single_line(&comment.text)
        ));
    }
}

fn status_label(status: crate::domain::review::ReviewStatus) -> &'static str {
    match status {
        crate::domain::review::ReviewStatus::Draft => "draft",
        crate::domain::review::ReviewStatus::Finished => "finished",
        crate::domain::review::ReviewStatus::Prepared => "prepared",
        crate::domain::review::ReviewStatus::Submitted => "submitted",
    }
}

/// Collapses a comment to one line so a listing cannot be reshaped by its text.
fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
