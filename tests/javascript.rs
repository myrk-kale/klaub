//! JavaScript, TypeScript and TSX extraction.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::format_collect,
    clippy::needless_raw_string_hashes
)]

use klaub::adapters::parse::TreeSitterParser;
use klaub::domain::graph::{EdgeOrigin, EntrypointKind, EntrypointScope};
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

const TS: &str = r#"
import { verify, sign as makeSig } from './tokens';
import Store, * as util from "../util";
import type { Config } from '@app/config';
export { helper } from './helper';
const legacy = require('./legacy');
const { a, b: c } = require("./pair");

export interface Session { id: string }
export type Token = string | number;
export enum Role { Admin, User }

export class SessionService extends Base {
  private store: Store;
  #secret = 1;

  constructor(store: Store) { super(); this.store = store; }

  async validate(token: string, strict = false): Promise<boolean> {
    if (strict && !token) { return false; }
    for (const part of token.split('.')) { verify(part); }
    return this.store.contains(token) ? true : false;
  }

  private helper(): void {}
  protected guard = (x: number) => x + 1;
}

export function topLevel(a: number, b = 2): number { return a + b; }
const internal = (x) => x * 2;
export const exported = async (x: number): Promise<number> => x;
function plain() {}
"#;

#[test]
fn typescript_symbols_signatures_and_visibility() {
    let file = parse(Language::TypeScript, TS);
    assert!(!file.has_syntax_errors);

    let service = symbol(&file, "SessionService");
    assert_eq!(service.kind, SymbolKind::Class);
    assert_eq!(service.visibility, Visibility::Public);
    assert_eq!(service.signature, "class SessionService extends Base");

    let validate = symbol(&file, "SessionService.validate");
    assert_eq!(validate.kind, SymbolKind::Method);
    assert!(validate.features.is_async);
    assert_eq!(
        validate.signature,
        "async validate(token: string, strict = false): Promise<boolean>"
    );
    assert_eq!(validate.features.param_count, 2);
    assert_eq!(validate.visibility, Visibility::Public);

    assert_eq!(
        symbol(&file, "SessionService.helper").visibility,
        Visibility::Private
    );
    assert_eq!(
        symbol(&file, "SessionService.guard").visibility,
        Visibility::Protected
    );
    assert_eq!(
        symbol(&file, "SessionService.guard").kind,
        SymbolKind::Method,
        "arrow functions in fields are methods"
    );

    assert_eq!(symbol(&file, "Session").kind, SymbolKind::Interface);
    assert_eq!(symbol(&file, "Token").kind, SymbolKind::TypeAlias);
    assert_eq!(symbol(&file, "Role").kind, SymbolKind::Enum);

    let top = symbol(&file, "topLevel");
    assert_eq!(top.visibility, Visibility::Public);
    assert_eq!(top.signature, "function topLevel(a: number, b = 2): number");

    let internal = symbol(&file, "internal");
    assert_eq!(internal.kind, SymbolKind::Function);
    assert_eq!(internal.visibility, Visibility::Private);
    assert_eq!(internal.signature, "const internal = (x) =>");

    let exported = symbol(&file, "exported");
    assert_eq!(exported.visibility, Visibility::Public);
    assert!(exported.features.is_async);
    assert_eq!(symbol(&file, "plain").visibility, Visibility::Private);
}

#[test]
fn export_wrappers_are_part_of_the_span() {
    let file = parse(Language::TypeScript, TS);
    let top = symbol(&file, "topLevel");
    let line = TS.lines().nth(top.span.start_line as usize - 1).unwrap();
    assert!(line.starts_with("export function topLevel"), "{line}");
}

#[test]
fn complexity_and_calls() {
    let file = parse(Language::TypeScript, TS);
    // 1 + if + && + for + ternary
    assert_eq!(
        symbol(&file, "SessionService.validate").features.complexity,
        5
    );
    let validate = file.find("SessionService.validate").unwrap();
    let calls: Vec<_> = file.calls.iter().filter(|c| c.caller == validate).collect();
    let verify = calls.iter().find(|c| c.name == "verify").unwrap();
    assert!(verify.in_loop);
    assert_eq!(verify.receiver, Receiver::None);
    let contains = calls.iter().find(|c| c.name == "contains").unwrap();
    assert_eq!(
        contains.receiver,
        Receiver::Path(vec!["this".into(), "store".into()])
    );
}

#[test]
fn imports_exports_and_requires() {
    let file = parse(Language::TypeScript, TS);
    let find = |module: &str| {
        file.imports
            .iter()
            .find(|i| i.module == module)
            .unwrap_or_else(|| panic!("no import of {module}"))
    };
    let tokens = find("./tokens");
    assert_eq!(
        tokens
            .names
            .iter()
            .map(|n| (n.name.as_str(), n.alias.as_deref()))
            .collect::<Vec<_>>(),
        vec![("verify", None), ("sign", Some("makeSig"))]
    );
    let util = find("../util");
    assert!(util.wildcard);
    assert_eq!(util.alias.as_deref(), Some("util"));
    assert_eq!(util.names[0].alias.as_deref(), Some("Store"));
    assert!(find("./helper").reexport);
    let legacy = find("./legacy");
    assert!(legacy.dynamic);
    assert_eq!(legacy.alias.as_deref(), Some("legacy"));
    let pair = find("./pair");
    assert_eq!(pair.names.len(), 2);
}

#[test]
fn public_exports_are_internal_entrypoints() {
    let file = parse(Language::TypeScript, TS);
    let exports: Vec<&str> = file
        .entrypoints
        .iter()
        .filter(|e| e.kind == EntrypointKind::PublicExport)
        .map(|e| file.symbols[e.symbol.unwrap()].qualified_name.as_str())
        .collect();
    assert!(
        exports.contains(&"topLevel")
            && exports.contains(&"SessionService")
            && exports.contains(&"exported")
    );
    assert!(!exports.contains(&"internal") && !exports.contains(&"plain"));
    assert!(
        file.entrypoints
            .iter()
            .all(|e| e.scope == EntrypointScope::Internal)
    );
}

#[test]
fn express_routes_become_external_entrypoints() {
    let source = r#"
const express = require('express');
const app = express();
const router = express.Router();

function listUsers(req, res) { res.json([]); }

app.get('/users', listUsers);
router.post('/login', async (req, res) => { await check(req.body); res.sendStatus(204); });
other.get('/maybe', (req, res) => {});
app.get('/no-handler');
app.listen(3000);
"#;
    let file = parse(Language::JavaScript, source);
    let route = |method: &str, path: &str| {
        file.entrypoints
            .iter()
            .find(|e| {
                e.kind
                    == EntrypointKind::Http {
                        method: method.into(),
                        route: path.into(),
                    }
            })
            .unwrap_or_else(|| panic!("no route {method} {path}: {:?}", file.entrypoints))
    };
    let users = route("GET", "/users");
    assert_eq!(users.origin, EdgeOrigin::Resolved);
    assert_eq!(file.symbols[users.symbol.unwrap()].name, "listUsers");
    assert_eq!(users.scope, EntrypointScope::External);

    let login = route("POST", "/login");
    assert_eq!(
        file.symbols[login.symbol.unwrap()].name,
        "POST /login",
        "inline handlers become symbols"
    );
    assert_eq!(login.origin, EdgeOrigin::Resolved);
    assert!(
        file.calls
            .iter()
            .any(|c| c.name == "check" && file.symbols[c.caller].name == "POST /login")
    );

    assert_eq!(route("GET", "/maybe").origin, EdgeOrigin::Heuristic);
    assert!(
        !file.entrypoints.iter().any(
            |e| matches!(&e.kind, EntrypointKind::Http { route, .. } if route == "/no-handler")
        )
    );
}

fn markers(language: Language, source: &str) -> Vec<(MarkerKind, u32)> {
    let file = parse(language, source);
    let mut all: Vec<_> = file
        .symbols
        .iter()
        .flat_map(|s| s.features.markers.iter().map(|m| (m.kind, m.line)))
        .collect();
    all.sort_by_key(|(_, line)| *line);
    all
}

#[test]
fn flags_risky_javascript_patterns() {
    let source = r#"
async function f(db, user, cmd, items) {
  try { await work(); } catch (e) {}
  try { await work(); } catch (e) { log(e); }
  db.query(`SELECT * FROM users WHERE name = '${user}'`);
  db.query("SELECT * FROM users WHERE name = '" + user + "'");
  db.query("SELECT * FROM users WHERE id = $1", [user]);
  eval(cmd);
  child_process.exec(cmd);
  https.request({ rejectUnauthorized: false });
  for (const i of items) { await fetch(i); }
}
const password = "hunter2hunter2";
const token = process.env.TOKEN;
const config = { apiKey: "s3cr3t-Value-9f8e", name: "a-long-readable-name" };
"#;
    let found = markers(Language::JavaScript, source);
    let lines = |kind: MarkerKind| {
        found
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, l)| *l)
            .collect::<Vec<_>>()
    };
    assert_eq!(lines(MarkerKind::SwallowedError), vec![3]);
    assert_eq!(lines(MarkerKind::RawSqlConcat), vec![5, 6]);
    assert_eq!(lines(MarkerKind::DynamicExecution), vec![8]);
    assert_eq!(lines(MarkerKind::ShellExecution), vec![9]);
    assert_eq!(lines(MarkerKind::VerificationDisabled), vec![10]);
    assert_eq!(lines(MarkerKind::CallInLoop), vec![11]);
    assert_eq!(lines(MarkerKind::SecretLiteral), vec![13, 15]);
}

#[test]
fn tsx_and_jsx_parse() {
    let tsx = "export function App({ name }: { name: string }) { return <div className=\"a\">{name}</div>; }\n";
    let file = parse(Language::Tsx, tsx);
    assert!(!file.has_syntax_errors);
    assert!(file.find("App").is_some());
    let jsx = "export const App = () => <p>hi</p>;\n";
    assert!(!parse(Language::JavaScript, jsx).has_syntax_errors);
}

#[test]
fn fingerprints_ignore_formatting_and_comments() {
    let one = parse(
        Language::TypeScript,
        "function f(a: number) {\n  // doubled\n  return a * 2;\n}\n",
    );
    let two = parse(
        Language::TypeScript,
        "function f(a: number) { return a*2; }\n",
    );
    let three = parse(
        Language::TypeScript,
        "function f(a: number) { return a * 3; }\n",
    );
    let token = |f: &ParsedFile| f.symbols[f.find("f").unwrap()].fingerprint.token_hash;
    assert_eq!(token(&one), token(&two));
    assert_ne!(token(&one), token(&three));
}

#[test]
fn broken_files_still_yield_recoverable_symbols() {
    let file = parse(
        Language::TypeScript,
        "function ok() { return 1; }\nfunction broken( {\nfunction also() { return 2; }\n",
    );
    assert!(file.has_syntax_errors);
    assert!(file.find("ok").is_some());
}

#[test]
fn highlighting_covers_the_basics() {
    use klaub::domain::symbol::HighlightClass;
    let source = "// hi\nexport const x = \"s\" + 42;\n";
    let spans = TreeSitterParser::new()
        .highlight(Language::TypeScript, source)
        .unwrap();
    let class_at = |needle: &str| {
        let start = source.find(needle).unwrap();
        spans
            .iter()
            .find(|s| s.start <= start && start < s.end)
            .map(|s| s.class)
    };
    assert_eq!(class_at("// hi"), Some(HighlightClass::Comment));
    assert_eq!(class_at("export"), Some(HighlightClass::Keyword));
    assert_eq!(class_at("\"s\""), Some(HighlightClass::String));
    assert_eq!(class_at("42"), Some(HighlightClass::Number));
}

#[test]
fn thrown_errors_are_recorded_by_constructor_name() {
    let file = parse(
        Language::TypeScript,
        r#"
export function load(id: string): string {
  if (!id) {
    throw new TypeError("id is required");
  }
  if (id === "x") {
    throw makeError("x");
  }
  throw lastError;
}
"#,
    );
    assert_eq!(
        symbol(&file, "load").features.raises,
        vec!["TypeError", "makeError", "lastError"]
    );
}
