//! `klaub flow`: status, context, validation and the prompt for the current snapshot.
//!
//! Nothing here contacts an agent. `prompt` only prints text; `validate` only reads.

use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use klaub_core::graph::SymbolKey;
use klaub_core::path::RepoPath;
use klaub_core::ports::FileReader;
use klaub_flow::{FlowContext, Validation};
use klaub_storage::FsFileReader;
use serde_json::json;

use crate::cli::FlowCommand;
use crate::context::Context;
use crate::output;

use super::worktree;

pub(super) fn run(ctx: &Context, command: FlowCommand) -> Result<ExitCode> {
    match command {
        FlowCommand::Status => status(ctx),
        FlowCommand::Context => context(ctx),
        FlowCommand::Validate { file } => validate(ctx, &file),
        FlowCommand::Prompt => prompt(ctx),
    }
}

fn status(ctx: &Context) -> Result<ExitCode> {
    let session = worktree::open_session(ctx)?;
    let flow = session.flow();
    output::json(&json!({
        "status": flow.state.label(),
        "path": flow.expected_path.as_str(),
        "issues": flow.issues,
    }))?;
    Ok(ExitCode::SUCCESS)
}

fn context(ctx: &Context) -> Result<ExitCode> {
    let session = worktree::open_session(ctx)?;
    let snapshot = &session.analysis().snapshot;
    let files: Vec<_> = snapshot
        .files
        .iter()
        .map(|file| {
            json!({
                "path": file.path.as_str(),
                "status": file.status,
            })
        })
        .collect();
    output::json(&json!({
        "snapshot_id": snapshot.id.as_str(),
        "base": snapshot.base_oid,
        "scope": snapshot.scope.label(),
        "files": files,
    }))?;
    Ok(ExitCode::SUCCESS)
}

fn validate(ctx: &Context, file: &Path) -> Result<ExitCode> {
    let session = worktree::open_session(ctx)?;
    let analysis = session.analysis();
    let bytes = FsFileReader.read_bounded(
        &analysis.snapshot.worktree.root,
        file,
        analysis.config.flows.max_file_bytes,
    )?;
    let known = |path: &RepoPath| {
        analysis.project_files.contains(path) || analysis.snapshot.file(path).is_some()
    };
    let symbols = |path: &RepoPath, name: &str| {
        analysis
            .parsed
            .get(path)
            .is_some_and(|parsed| parsed.find(name).is_some())
    };
    let connected = |from: &SymbolKey, to: &SymbolKey| analysis.graph.reaches(from, to, 6);
    let flow_context = FlowContext {
        snapshot_id: &analysis.snapshot.id,
        worktree_id: &analysis.snapshot.worktree.id,
        base_oid: &analysis.snapshot.base_oid,
        max_bytes: analysis.config.flows.max_file_bytes,
        knows_file: &known,
        symbol_exists: &symbols,
        connected: &connected,
    };
    let validation = klaub_flow::validate(&bytes, &flow_context);
    let issues = issues_of(&validation);
    for issue in &issues {
        output::line(issue);
    }
    output::line(validation.state().label());
    Ok(if issues.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn prompt(ctx: &Context) -> Result<ExitCode> {
    let session = worktree::open_session(ctx)?;
    output::line(&session.flow().request_prompt);
    Ok(ExitCode::SUCCESS)
}

/// Problems that make a document not valid for this snapshot.
fn issues_of(validation: &Validation) -> Vec<String> {
    match validation {
        Validation::Current(_) => Vec::new(),
        Validation::Stale { found, .. } => {
            vec![format!("the document belongs to snapshot {found}")]
        }
        Validation::Invalid(issues) => issues.clone(),
    }
}
