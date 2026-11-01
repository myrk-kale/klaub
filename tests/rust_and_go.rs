//! Rust and Go extraction.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::format_collect,
    clippy::needless_raw_string_hashes
)]

use klaub::adapters::parse::TreeSitterParser;
use klaub::domain::graph::{EdgeOrigin, EntrypointKind};
use klaub::domain::language::Language;
use klaub::domain::ports::SourceParser;
use klaub::domain::symbol::{MarkerKind, ParsedFile, Receiver, Symbol, SymbolKind, Visibility};

fn parse(language: Language, source: &str) -> ParsedFile {
    TreeSitterParser::new().parse(language, source).unwrap()
}

fn symbol<'a>(file: &'a ParsedFile, qualified: &str) -> &'a Symbol {
    file.symbols
        .iter()
        .find(|s| s.qualified_name == qualified)
        .unwrap_or_else(|| {
            panic!(
                "{qualified} not found in {:?}",
                file.symbols
                    .iter()
                    .map(|s| &s.qualified_name)
                    .collect::<Vec<_>>()
            )
        })
}

const RUST: &str = r#"
use std::collections::{HashMap, HashSet as Set};
use crate::auth::tokens::verify;
use super::util::*;

pub struct Session { id: u64 }

pub trait Store {
    fn get(&self, id: u64) -> Option<Session>;
}

impl Session {
    pub fn new(id: u64) -> Self { Self { id } }

    pub async fn validate(&self, token: &str, strict: bool) -> bool {
        if strict && token.is_empty() { return false; }
        for part in token.split('.') { verify(part); }
        match self.id { 0 => false, _ => true }
    }

    fn private_helper(&self) {}
}

impl Store for Session {
    fn get(&self, id: u64) -> Option<Session> { None }
}

pub(crate) fn crate_visible() {}

#[tokio::main]
async fn main() {
    let _ = std::fs::remove_file("x");
    unsafe { raw(); }
}

#[test]
fn it_works() {}

mod inner { pub fn nested() {} }
"#;

#[test]
fn rust_symbols_signatures_and_visibility() {
    let file = parse(Language::Rust, RUST);
    assert!(!file.has_syntax_errors);
    assert_eq!(symbol(&file, "Session").kind, SymbolKind::Struct);
    assert_eq!(symbol(&file, "Store").kind, SymbolKind::Trait);

    let new = symbol(&file, "Session.new");
    assert_eq!(new.kind, SymbolKind::Method);
    assert_eq!(new.visibility, Visibility::Public);
    assert_eq!(new.signature, "pub fn new(id: u64) -> Self");

    let validate = symbol(&file, "Session.validate");
    assert!(validate.features.is_async);
    assert_eq!(validate.features.param_count, 2);
    assert_eq!(
        validate.signature,
        "pub async fn validate(&self, token: &str, strict: bool) -> bool"
    );
    // 1 + if + && + for + 2 match arms
    assert_eq!(validate.features.complexity, 6);

    assert_eq!(
        symbol(&file, "Session.private_helper").visibility,
        Visibility::Private
    );
    assert_eq!(
        symbol(&file, "crate_visible").visibility,
        Visibility::Internal
    );
    assert_eq!(symbol(&file, "impl Session").kind, SymbolKind::Impl);
    assert_eq!(
        symbol(&file, "impl Store for Session").kind,
        SymbolKind::Impl
    );
    assert_eq!(
        symbol(&file, "Session.get").visibility,
        Visibility::Public,
        "trait implementations are public"
    );
    assert_eq!(symbol(&file, "inner").kind, SymbolKind::Module);
    assert_eq!(symbol(&file, "inner.nested").kind, SymbolKind::Function);
    assert!(symbol(&file, "it_works").features.is_test);
}

#[test]
fn rust_calls_imports_and_entrypoints() {
    let file = parse(Language::Rust, RUST);
    let validate = file.find("Session.validate").unwrap();
    let verify = file
        .calls
        .iter()
        .find(|c| c.caller == validate && c.name == "verify")
        .unwrap();
    assert!(verify.in_loop);
    assert_eq!(verify.receiver, Receiver::None);
    let remove = file.calls.iter().find(|c| c.name == "remove_file").unwrap();
    assert_eq!(
        remove.receiver,
        Receiver::Path(vec!["std".into(), "fs".into()])
    );

    let find = |module: &str, name: &str| {
        file.imports
            .iter()
            .find(|i| i.module == module && i.names.iter().any(|n| n.name == name))
    };
    assert!(find("std::collections", "HashMap").is_some());
    assert_eq!(
        find("std::collections", "HashSet").unwrap().names[0]
            .alias
            .as_deref(),
        Some("Set")
    );
    assert!(find("crate::auth::tokens", "verify").is_some());
    assert!(
        file.imports
            .iter()
            .any(|i| i.module == "super::util" && i.wildcard)
    );

    let main = file
        .entrypoints
        .iter()
        .find(|e| e.kind == EntrypointKind::Main)
        .unwrap();
    assert_eq!(file.symbols[main.symbol.unwrap()].name, "main");
    assert_eq!(main.origin, EdgeOrigin::Resolved);
}

#[test]
fn rust_markers() {
    let file = parse(
        Language::Rust,
        r#"
const API_TOKEN: &str = "s3cr3t-Value-9f8e";
fn f(db: Db, name: String) {
    let _ = std::fs::remove_file("x");
    unsafe { raw(); }
    db.query(format!("SELECT * FROM t WHERE n = '{}'", name));
    Command::new("sh").arg("-c");
    Client::builder().danger_accept_invalid_certs(true);
    for p in paths { std::fs::read_to_string(p); }
}
"#,
    );
    let kinds: Vec<MarkerKind> = file
        .symbols
        .iter()
        .flat_map(|s| s.features.markers.iter().map(|m| m.kind))
        .collect();
    for expected in [
        MarkerKind::SwallowedError,
        MarkerKind::UnsafeBlock,
        MarkerKind::RawSqlConcat,
        MarkerKind::ShellExecution,
        MarkerKind::VerificationDisabled,
        MarkerKind::CallInLoop,
        MarkerKind::SecretLiteral,
    ] {
        assert!(
            kinds.contains(&expected),
            "missing {expected:?} in {kinds:?}"
        );
    }
}

const GO: &str = r#"
package main

import (
    "fmt"
    "net/http"
    str "strings"
    _ "embed"
)

type Server struct { addr string }

type Handler interface { Serve() }

func NewServer(addr string) *Server { return &Server{addr: addr} }

func (s *Server) Handle(w http.ResponseWriter, r *http.Request, retries, limit int) {
    if r == nil && s != nil {
        return
    }
    for i := 0; i < retries; i++ {
        fmt.Println(str.ToUpper("x"))
    }
    switch limit {
    case 1:
        s.helper()
    case 2:
    }
}

func (s *Server) helper() {}

func main() {
    s := NewServer(":8080")
    http.HandleFunc("/users", users)
    s.Handle(nil, nil, 1, 2)
}

func users(w http.ResponseWriter, r *http.Request) {}

func TestThing(t *testing.T) {}
"#;

#[test]
fn go_symbols_calls_and_imports() {
    let file = parse(Language::Go, GO);
    assert!(
        !file.has_syntax_errors,
        "{:?}",
        file.symbols
            .iter()
            .map(|s| &s.qualified_name)
            .collect::<Vec<_>>()
    );
    assert_eq!(symbol(&file, "Server").kind, SymbolKind::Struct);
    assert_eq!(symbol(&file, "Handler").kind, SymbolKind::Interface);
    assert_eq!(symbol(&file, "NewServer").visibility, Visibility::Public);
    assert_eq!(symbol(&file, "users").visibility, Visibility::Private);

    let handle = symbol(&file, "Server.Handle");
    assert_eq!(handle.kind, SymbolKind::Method);
    assert_eq!(
        handle.features.param_count, 4,
        "`retries, limit int` declares two parameters"
    );
    // 1 + if + && + for + 2 cases
    assert_eq!(handle.features.complexity, 6);
    assert_eq!(
        symbol(&file, "Server.helper").visibility,
        Visibility::Private
    );
    assert!(symbol(&file, "TestThing").features.is_test);

    let index = file.find("Server.Handle").unwrap();
    let println = file
        .calls
        .iter()
        .find(|c| c.caller == index && c.name == "Println")
        .unwrap();
    assert!(println.in_loop);
    assert_eq!(println.receiver, Receiver::Path(vec!["fmt".into()]));

    let find = |module: &str| {
        file.imports
            .iter()
            .find(|i| i.module == module)
            .unwrap_or_else(|| panic!("no import of {module}"))
    };
    assert_eq!(find("net/http").alias, None);
    assert_eq!(find("strings").alias.as_deref(), Some("str"));
    assert_eq!(
        find("embed").alias,
        None,
        "blank imports have no usable alias"
    );
}

#[test]
fn go_entrypoints() {
    let file = parse(Language::Go, GO);
    let main = file
        .entrypoints
        .iter()
        .find(|e| e.kind == EntrypointKind::Main)
        .unwrap();
    assert_eq!(file.symbols[main.symbol.unwrap()].name, "main");
    let route = file
        .entrypoints
        .iter()
        .find(|e| matches!(&e.kind, EntrypointKind::Http { route, .. } if route == "/users"))
        .unwrap();
    assert_eq!(file.symbols[route.symbol.unwrap()].name, "users");
    assert_eq!(route.origin, EdgeOrigin::Heuristic);
}

#[test]
fn go_markers() {
    let file = parse(
        Language::Go,
        r#"
package db

func f(db *sql.DB, name string, paths []string) {
    db.Query("SELECT * FROM t WHERE n = '" + name + "'")
    db.Query(fmt.Sprintf("SELECT * FROM t WHERE n = '%s'", name))
    db.Query("SELECT * FROM t WHERE n = $1", name)
    cmd := exec.Command("bash", "-c", name)
    cfg := &tls.Config{InsecureSkipVerify: true}
    password := "hunter2hunter2"
    if err := run(); err != nil {
    }
    for _, p := range paths {
        os.ReadFile(p)
    }
}
"#,
    );
    let found: Vec<(MarkerKind, u32)> = file
        .symbols
        .iter()
        .flat_map(|s| s.features.markers.iter().map(|m| (m.kind, m.line)))
        .collect();
    let lines = |kind: MarkerKind| {
        found
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, l)| *l)
            .collect::<Vec<_>>()
    };
    assert_eq!(lines(MarkerKind::RawSqlConcat), vec![5, 6], "{found:?}");
    assert_eq!(lines(MarkerKind::ShellExecution), vec![8]);
    assert_eq!(lines(MarkerKind::VerificationDisabled), vec![9]);
    assert_eq!(lines(MarkerKind::SecretLiteral), vec![10]);
    assert_eq!(lines(MarkerKind::SwallowedError), vec![11]);
    assert_eq!(lines(MarkerKind::CallInLoop), vec![14]);
}

#[test]
fn rust_and_go_highlighting() {
    use klaub::domain::symbol::HighlightClass;
    let parser = TreeSitterParser::new();
    let rust = "// c\nfn f() -> u32 { 42 }\n";
    let spans = parser.highlight(Language::Rust, rust).unwrap();
    let class_at = |source: &str, spans: &[klaub::domain::symbol::HighlightSpan], needle: &str| {
        let start = source.find(needle).unwrap();
        spans
            .iter()
            .find(|s| s.start <= start && start < s.end)
            .map(|s| s.class)
    };
    assert_eq!(
        class_at(rust, &spans, "// c"),
        Some(HighlightClass::Comment)
    );
    assert_eq!(class_at(rust, &spans, "fn"), Some(HighlightClass::Keyword));
    assert_eq!(class_at(rust, &spans, "42"), Some(HighlightClass::Number));
    let go = "package p\n\n// c\nfunc F() string { return \"x\" }\n";
    let spans = parser.highlight(Language::Go, go).unwrap();
    assert_eq!(class_at(go, &spans, "func"), Some(HighlightClass::Keyword));
    assert_eq!(class_at(go, &spans, "\"x\""), Some(HighlightClass::String));
}

#[test]
fn every_supported_language_survives_garbage() {
    let parser = TreeSitterParser::new();
    for language in klaub::domain::language::Language::ALL {
        assert!(parser.supports(language));
        for source in ["", "}}}{{{ ((( \u{0}", "\x1b[31m", &"(".repeat(5000)] {
            let file = parser.parse(language, source).unwrap();
            assert!(!file.symbols.is_empty(), "the module symbol always exists");
        }
    }
}
