//! Python extraction: symbols, calls, imports, entrypoints, markers and fingerprints.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use klaub_core::graph::{EdgeOrigin, EntrypointKind};
use klaub_core::language::Language;
use klaub_core::ports::SourceParser;
use klaub_core::symbol::{MarkerKind, ParsedFile, Receiver, Symbol, SymbolKind, Visibility};
use klaub_parse::TreeSitterParser;

fn parse(source: &str) -> ParsedFile {
    TreeSitterParser::new()
        .parse(Language::Python, source)
        .unwrap()
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

const SERVICE: &str = r#"
import os
import numpy as np
from auth.tokens import verify, sign as make_sig
from . import helpers
from ..shared.util import *

class SessionService(Base):
    """Handles sessions."""

    def __init__(self, store):
        self.store = store

    @staticmethod
    def _internal(x: int) -> int:
        return x

    async def validate(self, token: str, strict: bool = False) -> bool:
        if strict and not token:
            return False
        for part in token.split("."):
            verify(part)
        return self.store.contains(token)

def top_level(a, b=2, *args, **kwargs):
    return a + b

@decorator(with_args=True)
def decorated():
    pass
"#;

#[test]
fn extracts_symbols_with_qualified_names_signatures_and_visibility() {
    let file = parse(SERVICE);
    assert!(!file.has_syntax_errors);
    assert_eq!(file.symbols[0].kind, SymbolKind::Module);
    assert_eq!(file.symbols[0].qualified_name, "<module>");

    let class = symbol(&file, "SessionService");
    assert_eq!(class.kind, SymbolKind::Class);
    assert_eq!(class.signature, "class SessionService(Base)");

    let init = symbol(&file, "SessionService.__init__");
    assert_eq!(init.kind, SymbolKind::Method);
    assert_eq!(
        init.visibility,
        Visibility::Public,
        "dunder names are public"
    );
    assert_eq!(init.features.param_count, 1, "self is not counted");

    let internal = symbol(&file, "SessionService._internal");
    assert_eq!(internal.visibility, Visibility::Private);
    assert_eq!(internal.decorators, vec!["staticmethod"]);
    assert_eq!(internal.signature, "def _internal(x: int) -> int");

    let validate = symbol(&file, "SessionService.validate");
    assert!(validate.features.is_async);
    assert_eq!(
        validate.signature,
        "async def validate(self, token: str, strict: bool = False) -> bool"
    );
    assert_eq!(validate.features.param_count, 2);
    assert_eq!(validate.parent, Some(file.find("SessionService").unwrap()));

    let top = symbol(&file, "top_level");
    assert_eq!(top.kind, SymbolKind::Function);
    assert_eq!(top.features.param_count, 4);

    let decorated = symbol(&file, "decorated");
    assert_eq!(decorated.decorators, vec!["decorator(with_args=True)"]);
    assert!(decorated.span.start_line < symbol(&file, "decorated").span.end_line);
    // The span starts at the decorator, not at `def`.
    let decorator_line = SERVICE
        .lines()
        .position(|l| l.starts_with("@decorator"))
        .unwrap() as u32
        + 1;
    assert_eq!(decorated.span.start_line, decorator_line);
}

#[test]
fn spans_cover_whole_definitions() {
    let file = parse(SERVICE);
    let validate = symbol(&file, "SessionService.validate");
    let lines: Vec<&str> = SERVICE.lines().collect();
    assert!(lines[validate.span.start_line as usize - 1].contains("async def validate"));
    assert!(lines[validate.span.end_line as usize - 1].contains("return self.store.contains"));
    let class = symbol(&file, "SessionService");
    assert!(
        class.span.start_line <= validate.span.start_line
            && class.span.end_line >= validate.span.end_line
    );
}

#[test]
fn complexity_counts_decision_points() {
    let file = parse(SERVICE);
    // 1 + if + `and` + for = 4
    assert_eq!(
        symbol(&file, "SessionService.validate").features.complexity,
        4
    );
    assert_eq!(symbol(&file, "top_level").features.complexity, 1);
    assert_eq!(
        symbol(&file, "SessionService.validate")
            .features
            .max_nesting,
        1
    );
}

#[test]
fn records_calls_with_their_receivers() {
    let file = parse(SERVICE);
    let validate = file.find("SessionService.validate").unwrap();
    let calls: Vec<_> = file.calls.iter().filter(|c| c.caller == validate).collect();
    let by_name = |name: &str| {
        calls
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no call to {name}"))
    };
    assert_eq!(by_name("verify").receiver, Receiver::None);
    assert!(by_name("verify").in_loop);
    assert_eq!(
        by_name("contains").receiver,
        Receiver::Path(vec!["self".into(), "store".into()])
    );
    assert_eq!(
        by_name("split").receiver,
        Receiver::Path(vec!["token".into()])
    );
}

#[test]
fn self_receivers_are_recognised() {
    let file =
        parse("class A:\n    def f(self):\n        self.g()\n    def g(self):\n        pass\n");
    let call = file.calls.iter().find(|c| c.name == "g").unwrap();
    assert_eq!(call.receiver, Receiver::SelfRef);
}

#[test]
fn records_every_import_form() {
    let file = parse(SERVICE);
    let find = |module: &str| {
        file.imports
            .iter()
            .find(|i| i.module == module)
            .unwrap_or_else(|| panic!("no import of {module}"))
    };
    assert_eq!(find("os").alias, None);
    assert_eq!(find("numpy").alias.as_deref(), Some("np"));
    let tokens = find("auth.tokens");
    assert_eq!(
        tokens
            .names
            .iter()
            .map(|n| (n.name.as_str(), n.alias.as_deref()))
            .collect::<Vec<_>>(),
        vec![("verify", None), ("sign", Some("make_sig"))]
    );
    let relative = file.imports.iter().find(|i| i.module.is_empty()).unwrap();
    assert_eq!(relative.relative_level, 1);
    assert_eq!(relative.names[0].name, "helpers");
    let wildcard = file
        .imports
        .iter()
        .find(|i| i.module == "shared.util")
        .unwrap();
    assert_eq!(wildcard.relative_level, 2);
    assert!(wildcard.wildcard);
}

#[test]
fn recognises_web_framework_routes_and_main_blocks() {
    let source = r#"
from fastapi import FastAPI, APIRouter
from flask import Flask

app = FastAPI()
router = APIRouter()
flask_app = Flask(__name__)

@app.get("/users/{id}")
def get_user(id): ...

@router.post("/login", status_code=200)
async def login(): ...

@flask_app.route("/items", methods=["POST", "GET"])
def items(): ...

@unknown.get("/maybe")
def maybe(): ...

def main():
    pass

if __name__ == "__main__":
    main()
"#;
    let file = parse(source);
    let http = |name: &str| {
        let index = file.find(name).unwrap();
        file.entrypoints
            .iter()
            .find(|e| e.symbol == Some(index))
            .unwrap_or_else(|| panic!("no entrypoint for {name}"))
    };
    let get_user = http("get_user");
    assert_eq!(
        get_user.kind,
        EntrypointKind::Http {
            method: "GET".into(),
            route: "/users/{id}".into()
        }
    );
    assert_eq!(get_user.origin, EdgeOrigin::Resolved);
    assert_eq!(
        http("login").kind,
        EntrypointKind::Http {
            method: "POST".into(),
            route: "/login".into()
        }
    );
    assert_eq!(
        http("items").kind,
        EntrypointKind::Http {
            method: "POST".into(),
            route: "/items".into()
        }
    );
    assert_eq!(
        http("maybe").origin,
        EdgeOrigin::Heuristic,
        "an unknown receiver is only a guess"
    );
    let main = file
        .entrypoints
        .iter()
        .find(|e| e.kind == EntrypointKind::Cli)
        .unwrap();
    assert_eq!(main.symbol, Some(0));
    assert_eq!(main.origin, EdgeOrigin::Resolved);
}

fn markers(source: &str) -> Vec<(MarkerKind, u32)> {
    let file = parse(source);
    let mut all: Vec<_> = file
        .symbols
        .iter()
        .flat_map(|s| s.features.markers.iter().map(|m| (m.kind, m.line)))
        .collect();
    all.sort_by_key(|(_, line)| *line);
    all
}

#[test]
fn flags_swallowed_errors() {
    let found = markers(
        "def f():\n    try:\n        risky()\n    except Exception:\n        pass\n\ndef g():\n    try:\n        risky()\n    except ValueError as e:\n        log(e)\n",
    );
    assert_eq!(
        found
            .iter()
            .filter(|(k, _)| *k == MarkerKind::SwallowedError)
            .count(),
        1
    );
}

#[test]
fn flags_sql_built_from_dynamic_text_but_not_parameterised_queries() {
    let source = r#"
def bad(cursor, user):
    cursor.execute(f"SELECT * FROM users WHERE name = '{user}'")
    cursor.execute("SELECT * FROM users WHERE name = '%s'" % user)
    cursor.execute("SELECT * FROM users WHERE id = " + user_id)
    cursor.execute("SELECT {} FROM t".format(col))

def good(cursor, user):
    cursor.execute("SELECT * FROM users WHERE name = %s", (user,))
    cursor.execute("SELECT 1 " + "FROM dual")
"#;
    let found = markers(source);
    let sql: Vec<u32> = found
        .iter()
        .filter(|(k, _)| *k == MarkerKind::RawSqlConcat)
        .map(|(_, l)| *l)
        .collect();
    assert_eq!(sql, vec![3, 4, 5, 6], "{found:?}");
}

#[test]
fn flags_secret_literals_without_flagging_placeholders() {
    let source = r#"
PASSWORD = "hunter2hunter2"
api_key = "your-api-key-here"
token = os.environ["TOKEN"]
client = Client(secret="s3cr3t-Value-9f8e")
def connect(password="Sup3rS3cretPw"): ...
name = "hunter2hunter2"
"#;
    let found = markers(source);
    let lines: Vec<u32> = found
        .iter()
        .filter(|(k, _)| *k == MarkerKind::SecretLiteral)
        .map(|(_, l)| *l)
        .collect();
    assert_eq!(lines, vec![2, 5, 6], "{found:?}");
}

#[test]
fn flags_shell_dynamic_execution_and_disabled_verification() {
    let source = r#"
import os, subprocess, requests
def run(cmd):
    os.system(cmd)
    subprocess.run(cmd, shell=True)
    subprocess.run(["ls"], shell=False)
    eval(cmd)
    requests.get(url, verify=False)
    requests.get(url, verify=True)
"#;
    let kinds: Vec<MarkerKind> = markers(source).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == MarkerKind::ShellExecution)
            .count(),
        2
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == MarkerKind::DynamicExecution)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == MarkerKind::VerificationDisabled)
            .count(),
        1
    );
}

#[test]
fn flags_io_inside_loops_only() {
    let source = "def f(urls):\n    requests.get('x')\n    for u in urls:\n        requests.get(u)\n    data = [open(p) for p in paths]\n";
    let lines: Vec<u32> = markers(source)
        .into_iter()
        .filter(|(k, _)| *k == MarkerKind::CallInLoop)
        .map(|(_, l)| l)
        .collect();
    assert_eq!(lines, vec![4, 5]);
}

#[test]
fn fingerprints_separate_formatting_renames_and_real_changes() {
    let original = parse("def f(a, b):\n    total = a + b  # sum\n    return total\n");
    let reformatted = parse("def f(a,b):\n    total   =   a+b\n\n    return total\n");
    let renamed = parse("def g(x, y):\n    result = x + y\n    return result\n");
    let changed = parse("def f(a, b):\n    total = a * b\n    return total\n");
    let fp = |file: &ParsedFile| {
        file.symbols[file.find("f").or(file.find("g")).unwrap()]
            .fingerprint
            .clone()
    };

    assert_eq!(
        fp(&original).token_hash,
        fp(&reformatted).token_hash,
        "formatting and comments do not change tokens"
    );
    assert_ne!(
        fp(&original).text_hash,
        fp(&reformatted).text_hash,
        "but the raw text differs"
    );
    assert_eq!(
        fp(&original).shape_hash,
        fp(&renamed).shape_hash,
        "renaming keeps the syntax shape"
    );
    assert_ne!(fp(&original).token_hash, fp(&renamed).token_hash);
    assert_ne!(fp(&original).token_hash, fp(&changed).token_hash);
}

#[test]
fn a_parent_fingerprint_ignores_its_nested_symbols() {
    let one = parse("class A:\n    x = 1\n    def m(self):\n        return 1\n");
    let two = parse("class A:\n    x = 1\n    def m(self):\n        return 2\n");
    assert_eq!(
        symbol(&one, "A").fingerprint.token_hash,
        symbol(&two, "A").fingerprint.token_hash
    );
    assert_ne!(
        symbol(&one, "A.m").fingerprint.token_hash,
        symbol(&two, "A.m").fingerprint.token_hash
    );
}

#[test]
fn syntax_errors_still_yield_the_recoverable_symbols() {
    let file = parse(
        "def ok():\n    return 1\n\ndef broken(:\n    pass\n\ndef also_ok():\n    return 2\n",
    );
    assert!(file.has_syntax_errors);
    assert!(file.find("ok").is_some());
    assert!(file.find("also_ok").is_some());
}

#[test]
fn hostile_inputs_do_not_crash_or_overflow_the_stack() {
    let deep = format!("x = {}1{}\n", "(".repeat(20_000), ")".repeat(20_000));
    let file = parse(&deep);
    assert!(file.has_syntax_errors || !file.symbols.is_empty());

    let wide = (0..2000)
        .map(|i| format!("def f{i}():\n    return {i}\n"))
        .collect::<String>();
    assert_eq!(parse(&wide).symbols.len(), 2001);

    assert_eq!(parse("").symbols.len(), 1);
    assert!(parse("\u{0}\u{1}\x1b[31m garbage \u{feff}").symbols.len() >= 1);
}

#[test]
fn highlighting_covers_keywords_strings_comments_and_numbers() {
    use klaub_core::symbol::HighlightClass;
    let source = "def f(): # note\n    return \"s\" + 42\n";
    let spans = TreeSitterParser::new()
        .highlight(Language::Python, source)
        .unwrap();
    let classes_of = |needle: &str| {
        let start = source.find(needle).unwrap();
        spans
            .iter()
            .filter(|s| s.start <= start && start < s.end)
            .map(|s| s.class)
            .collect::<Vec<_>>()
    };
    assert_eq!(classes_of("def"), vec![HighlightClass::Keyword]);
    assert_eq!(classes_of("# note"), vec![HighlightClass::Comment]);
    assert_eq!(classes_of("\"s\""), vec![HighlightClass::String]);
    assert_eq!(classes_of("42"), vec![HighlightClass::Number]);
    assert_eq!(classes_of("f()"), vec![HighlightClass::Function]);
    assert!(
        spans.windows(2).all(|w| w[0].end <= w[1].start),
        "spans must not overlap"
    );
}
