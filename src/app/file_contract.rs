//! The contract of files that are not source code: migrations, dependency manifests and
//! configuration.
//!
//! These files have no symbols, so the reader's usual unit is the whole file. What a reviewer wants
//! from them is still a contract: which tables does this migration create or drop, which
//! dependencies came and went, which settings changed. [`inspect_file`] answers that for the
//! formats it can read reliably (SQL and the common Python migration tools, `Cargo.toml`,
//! `package.json`, `pyproject.toml`, `requirements*.txt`, `go.mod`, JSON, TOML and `.env`), and
//! for the rest it says only what kind of file it is, so the textual diff stays the fallback.
//!
//! Values of settings are shown, except where the key looks like a secret or the file is an
//! environment file: there only the fact that a value changed is reported. Nothing here executes
//! or evaluates a file.

// Every name compared below is lowercased first, so the extension checks are not case-sensitive.
#![allow(clippy::case_sensitive_file_extension_comparisons)]

use std::collections::BTreeMap;

use serde_json::Value;

/// How many entries of one kind are listed. The rest are counted.
const MAX_LISTED: usize = 40;

/// Lock files: generated, and not read for a contract.
const LOCKS: &[&str] = &[
    "package-lock.json",
    "cargo.lock",
    "yarn.lock",
    "pnpm-lock.yaml",
    "composer.lock",
    "poetry.lock",
    "go.sum",
];

/// Key fragments that mark a value as a secret.
const SECRET_WORDS: &[&str] = &[
    "secret",
    "token",
    "password",
    "passwd",
    "apikey",
    "api_key",
    "private_key",
    "credential",
];

/// What kind of file this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A database migration.
    Migration,
    /// A manifest that declares dependencies.
    DependencyManifest,
    /// A settings file.
    Config,
}

impl FileKind {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Migration => "migration",
            Self::DependencyManifest => "dependency manifest",
            Self::Config => "configuration",
        }
    }
}

/// What changed in one non-code file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileContract {
    /// The file.
    pub path: String,
    /// What kind of file it is.
    pub kind: FileKind,
    /// Whether the content was read. `false`: only the kind is known and the textual diff is the
    /// fallback.
    pub parsed: bool,
    /// Things only the new version has.
    pub added: Vec<String>,
    /// Things only the old version had.
    pub removed: Vec<String>,
    /// Things both have whose value differs.
    pub changed: Vec<String>,
    /// Facts worth knowing, and the places where the reading was approximate or incomplete.
    pub notes: Vec<String>,
}

/// Reads the contract of a non-code file, or `None` when the file is not one of the kinds above.
///
/// `before` and `after` are the file's text on each side; either is `None` when the file did not
/// exist on that side.
pub fn inspect_file(path: &str, before: Option<&str>, after: Option<&str>) -> Option<FileContract> {
    let kind = classify(path)?;
    let mut contract = FileContract {
        path: path.to_owned(),
        kind,
        parsed: false,
        added: Vec::new(),
        removed: Vec::new(),
        changed: Vec::new(),
        notes: Vec::new(),
    };
    match kind {
        FileKind::Migration => migration(path, before, after, &mut contract),
        FileKind::DependencyManifest => manifest(path, before, after, &mut contract),
        FileKind::Config => config(path, before, after, &mut contract),
    }
    if before.is_none() {
        contract.notes.push("the file is new".to_owned());
    }
    if after.is_none() {
        contract.notes.push("the file was deleted".to_owned());
    }
    cap(&mut contract.added, &mut contract.notes, "added");
    cap(&mut contract.removed, &mut contract.notes, "removed");
    cap(&mut contract.changed, &mut contract.notes, "changed");
    Some(contract)
}

fn cap(list: &mut Vec<String>, notes: &mut Vec<String>, what: &str) {
    if list.len() > MAX_LISTED {
        let hidden = list.len() - MAX_LISTED;
        list.truncate(MAX_LISTED);
        notes.push(format!("{hidden} more {what} entries not listed"));
    }
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase()
}

fn classify(path: &str) -> Option<FileKind> {
    let lower = path.to_ascii_lowercase();
    if lower.starts_with(".klaub/") {
        return None;
    }
    let name = file_name(path);
    if LOCKS.contains(&name.as_str()) {
        return None;
    }
    if matches!(
        name.as_str(),
        "cargo.toml" | "package.json" | "pyproject.toml" | "go.mod"
    ) || (name.starts_with("requirements") && name.ends_with(".txt"))
    {
        return Some(FileKind::DependencyManifest);
    }
    let in_migrations = [
        "migrations/",
        "/migrate/",
        "alembic/versions/",
        "db/migrate/",
    ]
    .iter()
    .any(|dir| lower.contains(dir) || lower.starts_with(dir.trim_start_matches('/')));
    if name.ends_with(".sql") || (in_migrations && (name.ends_with(".py") || name.ends_with(".rb")))
    {
        return Some(FileKind::Migration);
    }
    if name.ends_with(".json")
        || name.ends_with(".toml")
        || name.ends_with(".yaml")
        || name.ends_with(".yml")
        || name.starts_with(".env")
        || name.ends_with(".env")
    {
        return Some(FileKind::Config);
    }
    None
}

// ---- migrations -------------------------------------------------------------------------------------------------

fn migration(path: &str, before: Option<&str>, after: Option<&str>, out: &mut FileContract) {
    let ops = |text: Option<&str>| -> Vec<String> {
        text.map(|t| {
            if path.to_ascii_lowercase().ends_with(".sql") {
                sql_operations(t)
            } else {
                tool_operations(t)
            }
        })
        .unwrap_or_default()
    };
    let (old, new) = (ops(before), ops(after));
    out.parsed = true;
    out.added = new.iter().filter(|o| !old.contains(o)).cloned().collect();
    out.removed = old.iter().filter(|o| !new.contains(o)).cloned().collect();
    if new.is_empty() && old.is_empty() {
        out.parsed = false;
        out.notes
            .push("no operation was recognised; read the file".to_owned());
    }
    let destructive = out
        .added
        .iter()
        .chain(out.removed.iter())
        .any(|op| op.contains("(drops data)"));
    if destructive {
        out.notes
            .push("an operation drops or empties data".to_owned());
    }
    if before.is_some() && after.is_some() {
        out.notes.push(
            "a migration that existed before was edited; copies that already ran will not see the edit"
                .to_owned(),
        );
    }
}

/// SQL statements this reader names, in file order.
fn sql_operations(text: &str) -> Vec<String> {
    let without_comments: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut ops = Vec::new();
    for statement in without_comments.split(';') {
        let tokens: Vec<&str> = statement.split_whitespace().collect();
        let lower: Vec<String> = tokens.iter().map(|t| t.to_ascii_lowercase()).collect();
        let name = |index: usize| -> String {
            tokens
                .get(index)
                .map_or("?", |t| t.trim_matches(['"', '`', '\'', '(', ')', ',']))
                .to_owned()
        };
        let skip_if = |mut index: usize, words: &[&str]| -> usize {
            if lower
                .get(index..index + words.len())
                .is_some_and(|w| w.iter().map(String::as_str).eq(words.iter().copied()))
            {
                index += words.len();
            }
            index
        };
        match lower
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["create", "table", ..] => {
                ops.push(format!(
                    "creates table {}",
                    name(skip_if(2, &["if", "not", "exists"]))
                ));
            }
            ["drop", "table", ..] => ops.push(format!(
                "drops table {} (drops data)",
                name(skip_if(2, &["if", "exists"]))
            )),
            ["truncate", ..] => {
                let at = if lower.get(1).map(String::as_str) == Some("table") {
                    2
                } else {
                    1
                };
                ops.push(format!("empties table {} (drops data)", name(at)));
            }
            ["delete", "from", ..] => {
                let table = name(2);
                if lower.iter().any(|t| t == "where") {
                    ops.push(format!("deletes rows from {table}"));
                } else {
                    ops.push(format!("deletes every row of {table} (drops data)"));
                }
            }
            ["create", "index" | "unique", ..] | ["create", _, "index", ..] => {
                if let Some(on) = lower.iter().position(|t| t == "on") {
                    ops.push(format!("creates an index on {}", name(on + 1)));
                } else {
                    ops.push("creates an index".to_owned());
                }
            }
            ["drop", "index", ..] => ops.push("drops an index".to_owned()),
            ["alter", "table", ..] => {
                let table_at = skip_if(2, &["only"]);
                let table_at = skip_if(table_at, &["if", "exists"]);
                let table = name(table_at);
                alter_table(&tokens, &lower, table_at + 1, &table, &mut ops);
            }
            _ => {}
        }
    }
    ops
}

fn alter_table(tokens: &[&str], lower: &[String], from: usize, table: &str, ops: &mut Vec<String>) {
    let clean = |index: usize| -> String {
        tokens
            .get(index)
            .map_or("?", |t| t.trim_matches(['"', '`', '\'', '(', ')', ',']))
            .to_owned()
    };
    let word = |index: usize| lower.get(index).map(String::as_str);
    let mut i = from;
    while i < lower.len() {
        match (word(i), word(i + 1)) {
            (Some("add"), Some("column")) => {
                let at = if word(i + 2) == Some("if") {
                    i + 5
                } else {
                    i + 2
                };
                ops.push(format!("adds column {table}.{}", clean(at)));
            }
            (Some("add"), Some("constraint" | "primary" | "foreign" | "unique" | "check")) => {
                ops.push(format!("adds a constraint on {table}"));
            }
            (Some("add"), Some(_)) if word(i + 1) != Some("column") => {
                ops.push(format!("adds column {table}.{}", clean(i + 1)));
            }
            (Some("drop"), Some("column")) => {
                let at = if word(i + 2) == Some("if") {
                    i + 4
                } else {
                    i + 2
                };
                ops.push(format!("drops column {table}.{} (drops data)", clean(at)));
            }
            (Some("drop"), Some("constraint")) => {
                ops.push(format!("drops a constraint on {table}"));
            }
            (Some("alter"), Some("column")) => {
                let column = clean(i + 2);
                if word(i + 3) == Some("type")
                    || (word(i + 3) == Some("set") && word(i + 4) == Some("data"))
                {
                    ops.push(format!("changes the type of {table}.{column}"));
                } else if word(i + 3) == Some("set") && word(i + 4) == Some("not") {
                    ops.push(format!("makes {table}.{column} required"));
                } else if word(i + 3) == Some("drop") && word(i + 4) == Some("not") {
                    ops.push(format!("makes {table}.{column} optional"));
                } else {
                    ops.push(format!("alters column {table}.{column}"));
                }
            }
            (Some("rename"), Some("to")) => {
                ops.push(format!("renames table {table} to {}", clean(i + 2)));
            }
            (Some("rename"), Some("column")) => {
                ops.push(format!(
                    "renames column {table}.{} to {}",
                    clean(i + 2),
                    clean(i + 4)
                ));
            }
            _ => {}
        }
        i += 1;
    }
}

/// Operations named by the common Python and Ruby migration tools (Alembic, Django, Rails).
fn tool_operations(text: &str) -> Vec<String> {
    const CALLS: &[(&str, &str, bool)] = &[
        ("op.create_table(", "creates table", false),
        ("op.drop_table(", "drops table", true),
        ("op.add_column(", "adds a column to", false),
        ("op.drop_column(", "drops a column from", true),
        ("op.alter_column(", "alters a column of", false),
        ("op.create_index(", "creates an index", false),
        ("op.drop_index(", "drops an index", false),
        ("op.execute(", "runs raw SQL", false),
        ("migrations.CreateModel(", "creates model", false),
        ("migrations.DeleteModel(", "deletes model", true),
        ("migrations.AddField(", "adds a field to", false),
        ("migrations.RemoveField(", "removes a field from", true),
        ("migrations.AlterField(", "alters a field of", false),
        ("migrations.RunSQL(", "runs raw SQL", false),
        ("migrations.RunPython(", "runs Python code", false),
        ("create_table", "creates table", false),
        ("drop_table", "drops table", true),
        ("add_column", "adds a column to", false),
        ("remove_column", "removes a column from", true),
    ];
    let mut ops = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        for (needle, label, destructive) in CALLS {
            if let Some(at) = trimmed.find(needle) {
                // The Rails helpers are bare words; require them at the start of the statement.
                if !needle.contains('.') && at != 0 {
                    continue;
                }
                let args = &trimmed[at + needle.len()..];
                let target = first_literal(args);
                let mut op = match target {
                    Some(name) if !label.starts_with("runs") && !label.contains("index") => {
                        format!("{label} {name}")
                    }
                    _ => (*label).to_owned(),
                };
                if *destructive {
                    op.push_str(" (drops data)");
                }
                ops.push(op);
                break;
            }
        }
    }
    ops
}

/// The first quoted word or `:symbol` in `text`.
fn first_literal(text: &str) -> Option<String> {
    let start = text.find(['"', '\'', ':'])?;
    let quote = text.as_bytes().get(start).copied()?;
    if quote == b':' {
        let word: String = text[start + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        return (!word.is_empty()).then_some(word);
    }
    let rest = &text[start + 1..];
    let end = rest.find(char::from(quote))?;
    let word = &rest[..end];
    (!word.is_empty()).then(|| word.to_owned())
}

// ---- dependency manifests ---------------------------------------------------------------------------------------

type Deps = BTreeMap<String, String>;

fn manifest(path: &str, before: Option<&str>, after: Option<&str>, out: &mut FileContract) {
    let name = file_name(path);
    let read = |text: Option<&str>| -> Result<Deps, String> {
        match text {
            None => Ok(Deps::new()),
            Some(text) => match name.as_str() {
                "cargo.toml" => cargo_dependencies(text),
                "package.json" => npm_dependencies(text),
                "pyproject.toml" => pyproject_dependencies(text),
                "go.mod" => Ok(go_dependencies(text)),
                _ => Ok(requirement_lines(text)),
            },
        }
    };
    match (read(before), read(after)) {
        (Ok(old), Ok(new)) => {
            out.parsed = true;
            diff_maps(&old, &new, out, "");
        }
        (Err(error), _) | (_, Err(error)) => {
            out.notes.push(format!(
                "could not read the dependencies ({error}); the textual diff is the fallback"
            ));
        }
    }
}

/// Compares two key-to-value maps into the added/removed/changed lists. Keys are
/// `section name` strings; the display is `name (section): value`.
fn diff_maps(old: &Deps, new: &Deps, out: &mut FileContract, hide_values_for: &str) {
    let show = |key: &str, value: &str| -> String {
        let hidden = !hide_values_for.is_empty() || is_secret_key(key);
        if hidden || value.is_empty() {
            key.to_owned()
        } else {
            format!("{key} {value}")
        }
    };
    for (key, value) in new {
        match old.get(key) {
            None => out.added.push(show(key, value)),
            Some(previous) if previous != value => {
                if !hide_values_for.is_empty() || is_secret_key(key) {
                    out.changed.push(format!("{key}: value changed (hidden)"));
                } else {
                    out.changed.push(format!("{key}: {previous} -> {value}"));
                }
            }
            Some(_) => {}
        }
    }
    for (key, value) in old {
        if !new.contains_key(key) {
            out.removed.push(show(key, value));
        }
    }
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_WORDS.iter().any(|word| lower.contains(word))
}

fn cargo_dependencies(text: &str) -> Result<Deps, String> {
    let root: toml::Table = toml::from_str(text).map_err(|e| e.message().to_owned())?;
    let mut deps = Deps::new();
    let mut take = |section: &str, table: Option<&toml::Value>| {
        let Some(table) = table.and_then(toml::Value::as_table) else {
            return;
        };
        for (name, value) in table {
            let version = match value {
                toml::Value::String(v) => v.clone(),
                toml::Value::Table(t) => t
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| {
                        t.get("path")
                            .and_then(toml::Value::as_str)
                            .map(|p| format!("path {p}"))
                    })
                    .or_else(|| {
                        t.get("git")
                            .and_then(toml::Value::as_str)
                            .map(|g| format!("git {g}"))
                    })
                    .or_else(|| {
                        t.get("workspace")
                            .and_then(toml::Value::as_bool)
                            .filter(|w| *w)
                            .map(|_| "workspace".to_owned())
                    })
                    .unwrap_or_default(),
                _ => String::new(),
            };
            deps.insert(format!("{name} [{section}]"), version);
        }
    };
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        take(section, root.get(section));
    }
    take(
        "workspace.dependencies",
        root.get("workspace").and_then(|w| w.get("dependencies")),
    );
    Ok(deps)
}

fn npm_dependencies(text: &str) -> Result<Deps, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut deps = Deps::new();
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(map) = root.get(section).and_then(Value::as_object) {
            for (name, version) in map {
                deps.insert(
                    format!("{name} [{section}]"),
                    version.as_str().unwrap_or_default().to_owned(),
                );
            }
        }
    }
    Ok(deps)
}

/// Splits a requirement such as `requests[socks]>=2.0; python_version<"4"` into name and spec.
fn requirement(spec: &str) -> Option<(String, String)> {
    let spec = spec.split('#').next().unwrap_or("").trim();
    let spec = spec.split(';').next().unwrap_or("").trim();
    if spec.is_empty() || spec.starts_with('-') {
        return None;
    }
    let end = spec
        .find(['=', '<', '>', '!', '~', ' ', '(', '@'])
        .unwrap_or(spec.len());
    let name = spec[..end]
        .split('[')
        .next()
        .unwrap_or("")
        .trim()
        .to_owned();
    if name.is_empty() {
        return None;
    }
    Some((name, spec[end..].trim().to_owned()))
}

fn requirement_lines(text: &str) -> Deps {
    text.lines()
        .filter_map(requirement)
        .map(|(name, spec)| (format!("{name} [requirements]"), spec))
        .collect()
}

fn pyproject_dependencies(text: &str) -> Result<Deps, String> {
    let root: toml::Table = toml::from_str(text).map_err(|e| e.message().to_owned())?;
    let mut deps = Deps::new();
    let mut take_array = |section: &str, value: Option<&toml::Value>| {
        for item in value.and_then(toml::Value::as_array).into_iter().flatten() {
            if let Some((name, spec)) = item.as_str().and_then(requirement) {
                deps.insert(format!("{name} [{section}]"), spec);
            }
        }
    };
    let project = root.get("project");
    take_array("project", project.and_then(|p| p.get("dependencies")));
    if let Some(optional) = project
        .and_then(|p| p.get("optional-dependencies"))
        .and_then(toml::Value::as_table)
    {
        for (group, value) in optional {
            take_array(&format!("optional:{group}"), Some(value));
        }
    }
    if let Some(poetry) = root
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        for (name, value) in poetry {
            let spec = match value {
                toml::Value::String(v) => v.clone(),
                toml::Value::Table(t) => t
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                _ => String::new(),
            };
            deps.insert(format!("{name} [poetry]"), spec);
        }
    }
    Ok(deps)
}

fn go_dependencies(text: &str) -> Deps {
    let mut deps = Deps::new();
    let mut in_block = false;
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        if line.starts_with("require (") {
            in_block = true;
            continue;
        }
        if in_block && line == ")" {
            in_block = false;
            continue;
        }
        let body = if in_block {
            Some(line)
        } else {
            line.strip_prefix("require ")
        };
        if let Some(body) = body {
            let mut parts = body.split_whitespace();
            if let (Some(module), Some(version)) = (parts.next(), parts.next()) {
                deps.insert(format!("{module} [require]"), version.to_owned());
            }
        }
    }
    deps
}

// ---- configuration ------------------------------------------------------------------------------------------------

fn config(path: &str, before: Option<&str>, after: Option<&str>, out: &mut FileContract) {
    let name = file_name(path);
    let is_env = name.starts_with(".env") || name.ends_with(".env");
    let read = |text: Option<&str>| -> Result<Deps, String> {
        let Some(text) = text else {
            return Ok(Deps::new());
        };
        if is_env {
            Ok(env_pairs(text))
        } else if name.ends_with(".json") {
            let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            let mut flat = Deps::new();
            flatten_json("", &value, &mut flat);
            Ok(flat)
        } else if name.ends_with(".toml") {
            let table: toml::Table = toml::from_str(text).map_err(|e| e.message().to_owned())?;
            let mut flat = Deps::new();
            flatten_toml("", &toml::Value::Table(table), &mut flat);
            Ok(flat)
        } else {
            Ok(yaml_pairs(text))
        }
    };
    match (read(before), read(after)) {
        (Ok(old), Ok(new)) => {
            out.parsed = true;
            diff_maps(&old, &new, out, if is_env { "env" } else { "" });
            if name.ends_with(".yaml") || name.ends_with(".yml") {
                out.notes.push(
                    "YAML was read line by line, so anchors, flow style and multi-line values are approximate"
                        .to_owned(),
                );
            }
            if is_env {
                out.notes
                    .push("environment values are never shown".to_owned());
            }
        }
        (Err(error), _) | (_, Err(error)) => out.notes.push(format!(
            "could not read the file ({error}); the textual diff is the fallback"
        )),
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => format!("{s:?}"),
        other => {
            let text = other.to_string();
            if text.chars().count() > 60 {
                let cut: String = text.chars().take(57).collect();
                format!("{cut}...")
            } else {
                text
            }
        }
    }
}

fn flatten_json(prefix: &str, value: &Value, out: &mut Deps) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, child) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_json(&path, child, out);
            }
        }
        other => {
            out.insert(prefix.to_owned(), scalar_text(other));
        }
    }
}

fn flatten_toml(prefix: &str, value: &toml::Value, out: &mut Deps) {
    match value {
        toml::Value::Table(table) if !table.is_empty() => {
            for (key, child) in table {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_toml(&path, child, out);
            }
        }
        other => {
            let json = serde_json::to_value(other).unwrap_or(Value::Null);
            out.insert(prefix.to_owned(), scalar_text(&json));
        }
    }
}

fn env_pairs(text: &str) -> Deps {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|line| {
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// Top-level and nested `key: value` pairs of simple YAML, by indentation. Lists and block scalars
/// are not followed.
fn yaml_pairs(text: &str) -> Deps {
    let mut deps = Deps::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for line in text.lines() {
        let content = line.split(" #").next().unwrap_or("");
        let trimmed = content.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        let indent = content.len() - content.trim_start().len();
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        while stack.last().is_some_and(|(i, _)| *i >= indent) {
            stack.pop();
        }
        let key = key.trim().trim_matches(['"', '\'']);
        let path = stack
            .iter()
            .map(|(_, k)| k.as_str())
            .chain(std::iter::once(key))
            .collect::<Vec<_>>()
            .join(".");
        let value = value.trim();
        if value.is_empty() {
            stack.push((indent, key.to_owned()));
        } else {
            deps.insert(path, value.to_owned());
        }
    }
    deps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(path: &str, before: Option<&str>, after: Option<&str>) -> FileContract {
        inspect_file(path, before, after).expect("a recognised file")
    }

    #[test]
    fn source_and_lock_files_are_not_handled_here() {
        assert!(inspect_file("src/main.rs", None, Some("fn main() {}")).is_none());
        assert!(inspect_file("Cargo.lock", None, Some("")).is_none());
        assert!(inspect_file(".klaub/reviews/a.json", None, Some("{}")).is_none());
    }

    #[test]
    fn a_new_sql_migration_lists_what_it_creates_and_drops() {
        let sql = "-- up\nCREATE TABLE IF NOT EXISTS users (id int);\nALTER TABLE users ADD COLUMN email text;\nALTER TABLE users DROP COLUMN legacy;\nDROP TABLE old_users;\nCREATE UNIQUE INDEX ix ON users (email);\nDELETE FROM sessions;\n";
        let c = contract("db/migrations/0002_users.sql", None, Some(sql));
        assert_eq!(c.kind, FileKind::Migration);
        assert!(c.parsed);
        assert_eq!(
            c.added,
            vec![
                "creates table users",
                "adds column users.email",
                "drops column users.legacy (drops data)",
                "drops table old_users (drops data)",
                "creates an index on users",
                "deletes every row of sessions (drops data)",
            ]
        );
        assert!(c.notes.iter().any(|n| n.contains("drops or empties data")));
        assert!(c.notes.iter().any(|n| n == "the file is new"));
    }

    #[test]
    fn a_delete_with_a_where_clause_is_not_flagged_as_dropping_data() {
        let c = contract(
            "migrations/1.sql",
            None,
            Some("DELETE FROM sessions WHERE expired = true;"),
        );
        assert_eq!(c.added, vec!["deletes rows from sessions"]);
        assert!(!c.notes.iter().any(|n| n.contains("drops or empties data")));
    }

    #[test]
    fn alembic_and_django_operations_are_named() {
        let alembic = "def upgrade():\n    op.create_table('accounts', sa.Column('id', sa.Integer))\n    op.drop_column('users', 'legacy')\n";
        let c = contract("alembic/versions/ab12_accounts.py", None, Some(alembic));
        assert_eq!(
            c.added,
            vec![
                "creates table accounts",
                "drops a column from users (drops data)"
            ]
        );

        let django =
            "operations = [\n    migrations.AddField(model_name='user', name='email'),\n]\n";
        let c = contract("app/migrations/0003_email.py", None, Some(django));
        assert_eq!(c.added, vec!["adds a field to user"]);
    }

    #[test]
    fn an_unrecognised_migration_falls_back_to_the_text() {
        let c = contract("db/migrations/x.sql", None, Some("VACUUM;"));
        assert!(!c.parsed);
        assert!(
            c.notes
                .iter()
                .any(|n| n.contains("no operation was recognised"))
        );
    }

    #[test]
    fn cargo_dependencies_come_go_and_change() {
        let before = "[dependencies]\nserde = \"1.0\"\nlog = { version = \"0.4\" }\n\n[dev-dependencies]\ntempfile = \"3\"\n";
        let after =
            "[dependencies]\nserde = \"1.1\"\nanyhow = \"1\"\nlocal = { path = \"../local\" }\n";
        let c = contract("Cargo.toml", Some(before), Some(after));
        assert_eq!(c.kind, FileKind::DependencyManifest);
        assert!(c.parsed);
        assert_eq!(
            c.added,
            vec![
                "anyhow [dependencies] 1",
                "local [dependencies] path ../local"
            ]
        );
        assert_eq!(
            c.removed,
            vec!["log [dependencies] 0.4", "tempfile [dev-dependencies] 3"]
        );
        assert_eq!(c.changed, vec!["serde [dependencies]: 1.0 -> 1.1"]);
    }

    #[test]
    fn npm_pyproject_requirements_and_go_are_read() {
        let c = contract(
            "package.json",
            Some(r#"{"dependencies":{"left-pad":"^1.0.0"}}"#),
            Some(r#"{"dependencies":{"left-pad":"^2.0.0"},"devDependencies":{"jest":"^29"}}"#),
        );
        assert_eq!(c.changed, vec!["left-pad [dependencies]: ^1.0.0 -> ^2.0.0"]);
        assert_eq!(c.added, vec!["jest [devDependencies] ^29"]);

        let c = contract(
            "pyproject.toml",
            Some("[project]\ndependencies = [\"requests>=2.0\"]\n"),
            Some("[project]\ndependencies = [\"requests>=2.31\", \"rich[jupyter]==13\"]\n"),
        );
        assert_eq!(c.changed, vec!["requests [project]: >=2.0 -> >=2.31"]);
        assert_eq!(c.added, vec!["rich [project] ==13"]);

        let c = contract(
            "requirements-dev.txt",
            Some("pytest==7\n# c\n-r base.txt\n"),
            Some("pytest==8\nblack\n"),
        );
        assert_eq!(c.changed, vec!["pytest [requirements]: ==7 -> ==8"]);
        assert_eq!(c.added, vec!["black [requirements]"]);

        let c = contract(
            "go.mod",
            Some("module x\n\nrequire (\n\tgithub.com/a/b v1.0.0\n)\n"),
            Some(
                "module x\n\nrequire (\n\tgithub.com/a/b v1.1.0\n)\nrequire github.com/c/d v0.1.0\n",
            ),
        );
        assert_eq!(
            c.changed,
            vec!["github.com/a/b [require]: v1.0.0 -> v1.1.0"]
        );
        assert_eq!(c.added, vec!["github.com/c/d [require] v0.1.0"]);
    }

    #[test]
    fn a_broken_manifest_is_reported_and_the_textual_diff_remains() {
        let c = contract("package.json", Some("{}"), Some("{ not json"));
        assert!(!c.parsed);
        assert!(
            c.notes
                .iter()
                .any(|n| n.contains("could not read the dependencies"))
        );
    }

    #[test]
    fn config_keys_are_compared_and_secrets_are_hidden() {
        let before = r#"{"server":{"port":80,"api_key":"old"},"debug":true}"#;
        let after = r#"{"server":{"port":8080,"api_key":"new","tls":true}}"#;
        let c = contract("config/app.json", Some(before), Some(after));
        assert_eq!(c.kind, FileKind::Config);
        assert_eq!(c.added, vec!["server.tls true"]);
        assert_eq!(c.removed, vec!["debug true"]);
        assert_eq!(
            c.changed,
            vec![
                "server.api_key: value changed (hidden)",
                "server.port: 80 -> 8080"
            ]
        );
    }

    #[test]
    fn env_files_never_show_values() {
        let c = contract(
            ".env.production",
            Some("DB_URL=postgres://u:p@h/db\nDEBUG=0\n"),
            Some("DB_URL=postgres://u:q@h/db\nSENTRY=abc\n"),
        );
        assert_eq!(c.added, vec!["SENTRY"]);
        assert_eq!(c.removed, vec!["DEBUG"]);
        assert_eq!(c.changed, vec!["DB_URL: value changed (hidden)"]);
        let shown = format!("{c:?}");
        assert!(!shown.contains("postgres"), "{shown}");
        assert!(!shown.contains("abc"), "{shown}");
    }

    #[test]
    fn yaml_is_read_approximately_and_says_so() {
        let c = contract(
            "deploy/values.yaml",
            Some("image:\n  tag: v1\nreplicas: 2\n"),
            Some("image:\n  tag: v2\nreplicas: 2\n"),
        );
        assert_eq!(c.changed, vec!["image.tag: v1 -> v2"]);
        assert!(c.notes.iter().any(|n| n.contains("approximate")));
    }

    #[test]
    fn long_lists_are_capped_and_counted() {
        let mut many = String::new();
        for i in 0..60 {
            many.push_str("pkg");
            many.push_str(&i.to_string());
            many.push_str("==1\n");
        }
        let c = contract("requirements.txt", None, Some(&many));
        assert_eq!(c.added.len(), MAX_LISTED);
        assert!(c.notes.iter().any(|n| n.contains("20 more added entries")));
    }
}
