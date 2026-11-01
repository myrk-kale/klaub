//! Executable architecture rules (see `docs/architecture.md`).
//!
//! The hexagonal layout is only worth having if it is enforced. Klaub is a single crate, so the
//! layers are modules and the compiler does not keep them apart. These tests read the sources and
//! fail CI when a module reaches for something it must not know about, instead of letting the
//! violation survive review.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

/// Layers that own their directory under `src/`.
const LAYERS: &[&str] = &["domain", "services", "app", "adapters", "tui", "cli"];

/// APIs that reach outside the process. They belong in adapters, behind ports.
const FORBIDDEN_IO_APIS: &[&str] = &[
    "std::fs",
    "std::process",
    "std::net",
    "std::env",
    "std::thread::sleep",
    "std::time::SystemTime",
    "std::time::Instant",
    "OffsetDateTime::now",
    "tokio::",
    "reqwest",
    "libc::",
];

/// A module of the crate: its layer and, for layers made of independent parts, the part.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    layer: String,
    part: Option<String>,
}

impl Place {
    fn describe(&self) -> String {
        match &self.part {
            Some(part) => format!("{}::{part}", self.layer),
            None => self.layer.clone(),
        }
    }
}

/// Layers whose children never know about each other, except where `may_use` says so.
fn has_parts(layer: &str) -> bool {
    matches!(layer, "services" | "adapters")
}

/// Whether code in `from` may name `to`.
///
/// Dependencies only point inwards: `cli -> tui -> app -> services -> domain`, and
/// `adapters -> domain`. The command line is the composition root and may name anything.
fn may_use(from: &Place, to: &Place) -> bool {
    if from.layer == "cli" {
        return true;
    }
    if from.layer == to.layer {
        // Inside a layer made of parts, a part stays on its own, with one declared exception.
        return !has_parts(&from.layer)
            || from.part == to.part
            || to.part.is_none()
            || (from.layer == "services"
                && from.part.as_deref() == Some("rules")
                && to.part.as_deref() == Some("graph"));
    }
    let allowed: &[&str] = match from.layer.as_str() {
        "domain" | "adapters" | "services" => &["domain"],
        "app" => &["domain", "services"],
        "tui" => &["domain", "app"],
        _ => &[],
    };
    allowed.contains(&to.layer.as_str())
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn place_of(file: &Path) -> Option<Place> {
    let relative = file.strip_prefix(src_dir()).ok()?;
    let mut parts = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy());
    let layer = parts.next()?.into_owned();
    if !LAYERS.contains(&layer.as_str()) {
        return None; // lib.rs and main.rs wire the layers together
    }
    let part = if has_parts(&layer) {
        parts
            .next()
            .map(|p| p.trim_end_matches(".rs").to_owned())
            .filter(|p| p != "mod")
    } else {
        None
    };
    Some(Place { layer, part })
}

/// The code of a file that ships: comments and the trailing `#[cfg(test)] mod tests` are dropped.
fn shipped_code(text: &str) -> Vec<(usize, &str)> {
    let all: Vec<&str> = text.lines().collect();
    let mut lines = Vec::new();
    for (index, line) in all.iter().enumerate() {
        let starts_tests = line.trim() == "#[cfg(test)]"
            && all
                .get(index + 1)
                .is_some_and(|next| next.trim_start().starts_with("mod tests"));
        if starts_tests {
            break;
        }
        let code = line.split("//").next().unwrap_or("");
        lines.push((index + 1, code));
    }
    lines
}

/// Places named by `crate::<layer>[::<part>]` in one line of code.
fn mentioned_places(code: &str) -> Vec<Place> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("crate::") {
        rest = &rest[at + "crate::".len()..];
        let mut segments = rest
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
            .next()
            .unwrap_or("")
            .split("::");
        let Some(layer) = segments.next().filter(|l| LAYERS.contains(l)) else {
            continue;
        };
        let part = if has_parts(layer) {
            segments.next().filter(|p| !p.is_empty()).map(str::to_owned)
        } else {
            None
        };
        found.push(Place {
            layer: layer.to_owned(),
            part,
        });
    }
    found
}

fn all_sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    rust_sources(&src_dir(), &mut files);
    files.sort();
    files
}

#[test]
fn dependencies_point_inwards() {
    let mut violations = Vec::new();
    for file in all_sources() {
        let Some(from) = place_of(&file) else {
            continue;
        };
        let text = fs::read_to_string(&file).expect("read source");
        for (number, code) in shipped_code(&text) {
            for to in mentioned_places(code) {
                if !may_use(&from, &to) {
                    violations.push(format!(
                        "{}:{number}: {} -> {}",
                        file.strip_prefix(src_dir()).unwrap().display(),
                        from.describe(),
                        to.describe()
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "forbidden dependencies between layers:\n  {}\nSee docs/architecture.md for the allowed directions.",
        violations.join("\n  ")
    );
}

#[test]
fn only_the_composition_root_names_adapters() {
    let mut offences = Vec::new();
    for file in all_sources() {
        let Some(from) = place_of(&file) else {
            continue;
        };
        if matches!(from.layer.as_str(), "cli" | "adapters") {
            continue;
        }
        let text = fs::read_to_string(&file).expect("read source");
        for (number, code) in shipped_code(&text) {
            if mentioned_places(code).iter().any(|p| p.layer == "adapters") {
                offences.push(format!(
                    "{}:{number}",
                    file.strip_prefix(src_dir()).unwrap().display()
                ));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "only `cli` may name adapters, but these do:\n  {}",
        offences.join("\n  ")
    );
}

#[test]
fn every_source_file_belongs_to_a_layer() {
    for file in all_sources() {
        let relative = file.strip_prefix(src_dir()).unwrap();
        let top = relative
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy();
        assert!(
            LAYERS.contains(&top.as_ref()) || ["lib.rs", "main.rs"].contains(&top.as_ref()),
            "src/{} is not in a layer; add the layer to tests/architecture.rs",
            relative.display()
        );
    }
}

#[test]
fn inner_layers_perform_no_io() {
    let mut offences = Vec::new();
    for file in all_sources() {
        let Some(place) = place_of(&file) else {
            continue;
        };
        if !matches!(place.layer.as_str(), "domain" | "services" | "app") {
            continue;
        }
        let text = fs::read_to_string(&file).expect("read source");
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for api in FORBIDDEN_IO_APIS {
                if code.contains(api) {
                    offences.push(format!(
                        "src/{}:{}: uses `{api}`",
                        file.strip_prefix(src_dir()).unwrap().display(),
                        number + 1
                    ));
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "I/O belongs in adapters behind ports:\n  {}",
        offences.join("\n  ")
    );
}

#[test]
fn the_scanner_finds_what_it_is_meant_to_find() {
    // Guards the guard: a broken regex would make every rule above pass vacuously.
    let places =
        mentioned_places("use crate::adapters::git::GitRunner; use crate::domain::Result;");
    assert_eq!(places.len(), 2);
    assert_eq!(places[0].describe(), "adapters::git");
    assert_eq!(places[1].describe(), "domain");
    assert!(!may_use(&places[1], &places[0]));
    assert!(may_use(&places[0], &places[1]));
    let rules = Place {
        layer: "services".into(),
        part: Some("rules".into()),
    };
    let graph = Place {
        layer: "services".into(),
        part: Some("graph".into()),
    };
    let matching = Place {
        layer: "services".into(),
        part: Some("matching".into()),
    };
    assert!(may_use(&rules, &graph));
    assert!(!may_use(&matching, &graph));
    assert!(all_sources().len() > 50, "sources were not found");
}
