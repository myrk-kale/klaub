//! The message catalog is checked by the compiler for missing languages and misspelt
//! placeholders. These tests cover what the compiler cannot see: a translation that forgets a
//! placeholder, or one that changes the whitespace the layout depends on.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// One entry of the catalog: its name and the text it has in each language.
struct Entry {
    name: String,
    en: String,
    pt: String,
    es: String,
}

fn catalog(file: &str) -> Vec<Entry> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/domain/i18n")
        .join(file);
    let text = fs::read_to_string(path).expect("read the catalog");
    let mut entries = Vec::new();
    let mut name = String::new();
    let mut texts: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line
            .strip_prefix("fn ")
            .or_else(|| line.strip_prefix("const "))
        {
            rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap()
                .clone_into(&mut name);
            texts.clear();
        }
        for language in ["en", "pt", "es"] {
            if let Some(rest) = line.strip_prefix(&format!("{language}: \"")) {
                let literal = rest.strip_suffix("\",").expect("a closed literal");
                texts.push((language.to_owned(), literal.to_owned()));
            }
        }
        if texts.len() == 3 {
            entries.push(Entry {
                name: name.clone(),
                en: texts[0].1.clone(),
                pt: texts[1].1.clone(),
                es: texts[2].1.clone(),
            });
            texts.clear();
        }
    }
    entries
}

/// The `{name}` / `{name:fmt}` fields of a format string, without the format specs.
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('{', Some('{')) => {
                chars.next();
            }
            ('{', _) => {
                let field: String = chars.by_ref().take_while(|&c| c != '}').collect();
                found.insert(field.split(':').next().unwrap_or_default().to_owned());
            }
            _ => {}
        }
    }
    found
}

fn edges(text: &str) -> (usize, usize) {
    (
        text.len() - text.trim_start().len(),
        text.len() - text.trim_end().len(),
    )
}

#[test]
fn the_catalog_is_parsed_completely() {
    // A rough floor: if the parser above stops matching entries this fails instead of passing
    // vacuously.
    assert!(catalog("tui.rs").len() > 400);
    assert!(catalog("cli.rs").len() > 60);
}

#[test]
fn every_translation_mentions_the_same_values_as_the_english_text() {
    for file in ["tui.rs", "cli.rs"] {
        for entry in catalog(file) {
            let wanted = placeholders(&entry.en);
            for (language, text) in [("pt", &entry.pt), ("es", &entry.es)] {
                assert_eq!(
                    placeholders(text),
                    wanted,
                    "{file}: `{}` in {language} must use exactly the placeholders of the English text",
                    entry.name
                );
            }
        }
    }
}

#[test]
fn translations_keep_the_spaces_around_the_text() {
    // Some sentences are joined to others or aligned in columns, so a stray space shifts them.
    for file in ["tui.rs", "cli.rs"] {
        for entry in catalog(file) {
            for (language, text) in [("pt", &entry.pt), ("es", &entry.es)] {
                assert_eq!(
                    edges(text),
                    edges(&entry.en),
                    "{file}: `{}` in {language} must start and end with the same spaces as the English text",
                    entry.name
                );
            }
        }
    }
}

#[test]
fn no_translation_is_empty_or_a_leftover_of_a_template() {
    for file in ["tui.rs", "cli.rs"] {
        for entry in catalog(file) {
            for (language, text) in [("pt", &entry.pt), ("es", &entry.es)] {
                assert!(
                    !text.trim().is_empty(),
                    "{file}: `{}` in {language} is empty",
                    entry.name
                );
                assert!(
                    !text.contains("TODO") && !text.contains("FIXME"),
                    "{file}: `{}` in {language} is unfinished",
                    entry.name
                );
            }
        }
    }
}
