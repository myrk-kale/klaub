//! The graph over small multi-file projects, parsed with the real parser.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::format_collect)]

use std::sync::Arc;

use klaub::adapters::parse::TreeSitterParser;
use klaub::domain::graph::{EdgeOrigin, EntrypointKind, EntrypointScope, ImpactScope, SymbolKey};
use klaub::domain::language::Language;
use klaub::domain::path::RepoPath;
use klaub::domain::ports::SourceParser;
use klaub::services::graph::{
    ImpactOptions, IndexedFile, ManifestText, ModuleMap, ProjectGraph, build_graph,
};

fn project(files: &[(&str, &str)], manifests: &[(&str, &str)]) -> ProjectGraph {
    let parser = TreeSitterParser::new();
    let indexed = files
        .iter()
        .map(|(path, source)| {
            let path = RepoPath::new(path).unwrap();
            let language = Language::from_path(&path).expect("a supported language");
            IndexedFile {
                parsed: Arc::new(parser.parse(language, source).unwrap()),
                path,
            }
        })
        .collect();
    let manifests: Vec<ManifestText> = manifests
        .iter()
        .map(|(p, t)| ManifestText {
            path: RepoPath::new(p).unwrap(),
            text: (*t).to_owned(),
        })
        .collect();
    build_graph(indexed, &manifests)
}

fn key(file: &str, name: &str) -> SymbolKey {
    SymbolKey::new(RepoPath::new(file).unwrap(), name)
}

fn impact(graph: &ProjectGraph, file: &str, name: &str) -> klaub::domain::graph::ImpactReport {
    graph
        .impact(
            &key(file, name),
            &ImpactOptions::default(),
            &ModuleMap::default(),
        )
        .unwrap_or_else(|| panic!("{file}::{name} is not in the graph"))
}

fn callers(report: &klaub::domain::graph::ImpactReport, depth: u32) -> Vec<String> {
    report
        .callers
        .iter()
        .find(|l| l.depth == depth)
        .map(|l| l.callers.iter().map(|c| c.symbol.to_string()).collect())
        .unwrap_or_default()
}

const PYTHON_PROJECT: &[(&str, &str)] = &[
    ("src/app/__init__.py", ""),
    ("src/auth/__init__.py", ""),
    (
        "src/app/main.py",
        "from .service import handle\n\ndef run():\n    handle(1)\n\nif __name__ == \"__main__\":\n    run()\n",
    ),
    (
        "src/app/service.py",
        "import json\nfrom ..auth.tokens import verify\nfrom ..auth import tokens as tk\n\ndef handle(x):\n    v = verify(x)\n    tk.refresh(x)\n    helper(v)\n    return json.dumps(v)\n\ndef helper(v):\n    return v\n",
    ),
    (
        "src/auth/tokens.py",
        "def verify(t):\n    return decode(t)\n\ndef decode(t):\n    return t\n\ndef refresh(t):\n    return t\n",
    ),
    (
        "src/api/routes.py",
        "from fastapi import FastAPI\nfrom app.service import handle\n\napp = FastAPI()\n\n@app.post(\"/login\")\ndef login(req):\n    return handle(req)\n",
    ),
];

#[test]
fn calls_are_resolved_through_scope_and_imports() {
    let graph = project(PYTHON_PROJECT, &[]);
    let verify = impact(&graph, "src/auth/tokens.py", "verify");
    assert_eq!(callers(&verify, 1), vec!["src/app/service.py › handle"]);
    assert_eq!(verify.callers[0].callers[0].origin, EdgeOrigin::Resolved);
    assert_eq!(
        verify
            .callees
            .iter()
            .map(|c| c.symbol.qualified_name.as_str())
            .collect::<Vec<_>>(),
        vec!["decode"]
    );

    // `tk.refresh` goes through `from ..auth import tokens as tk` (a submodule binding).
    let refresh = impact(&graph, "src/auth/tokens.py", "refresh");
    assert_eq!(callers(&refresh, 1), vec!["src/app/service.py › handle"]);
    assert_eq!(refresh.callers[0].callers[0].origin, EdgeOrigin::Resolved);

    // A call to a function of the same file.
    assert_eq!(
        callers(&impact(&graph, "src/app/service.py", "helper"), 1),
        vec!["src/app/service.py › handle"]
    );
}

#[test]
fn external_calls_neither_create_edges_nor_count_as_unresolved() {
    let graph = project(PYTHON_PROJECT, &[]);
    assert_eq!(
        graph.unresolved_calls(),
        0,
        "json.dumps and friends are simply outside the project"
    );
}

#[test]
fn impact_walks_up_to_every_kind_of_entrypoint() {
    let graph = project(PYTHON_PROJECT, &[]);
    let decode = impact(&graph, "src/auth/tokens.py", "decode");
    assert_eq!(callers(&decode, 1), vec!["src/auth/tokens.py › verify"]);
    assert_eq!(callers(&decode, 2), vec!["src/app/service.py › handle"]);
    let level3 = callers(&decode, 3);
    assert!(
        level3.contains(&"src/api/routes.py › login".to_owned())
            && level3.contains(&"src/app/main.py › run".to_owned()),
        "{level3:?}"
    );

    let http = decode
        .entrypoints
        .iter()
        .find(|e| matches!(e.entrypoint.kind, EntrypointKind::Http { .. }))
        .unwrap();
    assert_eq!(http.entrypoint.scope, EntrypointScope::External);
    assert_eq!(
        http.entrypoint.kind,
        EntrypointKind::Http {
            method: "POST".into(),
            route: "/login".into()
        }
    );
    let path: Vec<&str> = http
        .path
        .iter()
        .map(|s| s.symbol.qualified_name.as_str())
        .collect();
    assert_eq!(
        path,
        vec!["login", "handle", "verify", "decode"],
        "from the entrypoint down to the changed symbol"
    );
    assert_eq!(http.origin, EdgeOrigin::Resolved);

    let cli = decode
        .entrypoints
        .iter()
        .find(|e| e.entrypoint.kind == EntrypointKind::Cli)
        .unwrap();
    assert_eq!(
        cli.entrypoint.symbol.qualified_name, "<module>",
        "the `__main__` block belongs to the module"
    );
    assert_eq!(
        decode.scope,
        ImpactScope::Global,
        "several modules and an external entrypoint"
    );
}

#[test]
fn a_symbol_that_is_itself_an_entrypoint_reaches_itself() {
    let graph = project(PYTHON_PROJECT, &[]);
    let login = impact(&graph, "src/api/routes.py", "login");
    assert_eq!(login.entrypoints.len(), 1);
    assert_eq!(login.entrypoints[0].path.len(), 1);
    assert_eq!(login.scope, ImpactScope::Local, "nothing calls it");
}

#[test]
fn self_calls_are_resolved_and_receivers_of_unknown_type_are_guesses() {
    let graph = project(
        &[
            (
                "src/models/session.py",
                "class Session:\n    def __init__(self):\n        self.ready = True\n\n    def validate(self):\n        return self.check()\n\n    def check(self):\n        return self.ready\n",
            ),
            (
                "src/api/views.py",
                "from models.session import Session\n\ndef view():\n    s = Session()\n    return s.validate()\n",
            ),
        ],
        &[],
    );
    let check = impact(&graph, "src/models/session.py", "Session.check");
    assert_eq!(
        check.callers[0].callers[0].symbol.qualified_name,
        "Session.validate"
    );
    assert_eq!(
        check.callers[0].callers[0].origin,
        EdgeOrigin::Resolved,
        "self.check() is unambiguous"
    );

    // `s.validate()`: the type of `s` is not known, so the edge is a name-based guess.
    let validate = impact(&graph, "src/models/session.py", "Session.validate");
    let from_view = validate.callers[0]
        .callers
        .iter()
        .find(|c| c.symbol.qualified_name == "view")
        .unwrap();
    assert_eq!(from_view.origin, EdgeOrigin::Heuristic);

    // Instantiating the class reaches the class and its constructor.
    let init = impact(&graph, "src/models/session.py", "Session.__init__");
    assert_eq!(init.callers[0].callers[0].symbol.qualified_name, "view");
    assert_eq!(init.callers[0].callers[0].origin, EdgeOrigin::Resolved);
}

#[test]
fn inherited_methods_are_found_through_the_base_class() {
    let graph = project(
        &[(
            "app.py",
            "class Base:\n    def audit(self):\n        return 1\n\nclass Child(Base):\n    def act(self):\n        return self.audit()\n",
        )],
        &[],
    );
    let audit = impact(&graph, "app.py", "Base.audit");
    assert_eq!(
        audit.callers[0].callers[0].symbol.qualified_name,
        "Child.act"
    );
    assert_eq!(audit.callers[0].callers[0].origin, EdgeOrigin::Resolved);
}

#[test]
fn ambiguous_names_are_counted_not_followed() {
    let mut files: Vec<(String, String)> = (0..6)
        .map(|i| {
            (
                format!("impl{i}.py"),
                "class H:\n    def process(self):\n        return 1\n".to_owned(),
            )
        })
        .collect();
    files.push((
        "use.py".to_owned(),
        "def go(thing):\n    return thing.process()\n".to_owned(),
    ));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    let graph = project(&refs, &[]);
    assert_eq!(
        graph.call_edge_count(),
        0,
        "six candidates are too many to guess between"
    );
    assert_eq!(graph.unresolved_calls(), 1);
    let report = impact(&graph, "impl0.py", "H.process");
    assert!(report.callers.is_empty());
    assert_eq!(
        report.coverage.unresolved_calls, 0,
        "the unresolved call is in `use.py`, outside the visited area"
    );
}

#[test]
fn recursion_and_cycles_terminate() {
    let graph = project(
        &[(
            "a.py",
            "def a():\n    return b()\n\ndef b():\n    return a()\n\ndef c():\n    return c()\n",
        )],
        &[],
    );
    let a = impact(&graph, "a.py", "a");
    assert_eq!(callers(&a, 1), vec!["a.py › b"]);
    assert!(callers(&a, 2).is_empty(), "a is already visited");
    assert!(
        impact(&graph, "a.py", "c").callers.is_empty(),
        "self-calls are ignored"
    );
}

#[test]
fn the_depth_limit_is_reported() {
    let source: String = (0..12)
        .map(|i| format!("def f{i}():\n    return f{}()\n\n", i + 1))
        .collect::<String>()
        + "def f12():\n    return 1\n";
    let graph = project(&[("chain.py", &source)], &[]);
    let report = graph
        .impact(
            &key("chain.py", "f12"),
            &ImpactOptions {
                max_depth: 3,
                max_entrypoints: 5,
            },
            &ModuleMap::default(),
        )
        .unwrap();
    assert_eq!(report.callers.len(), 3);
    assert!(report.coverage.truncated);
    let full = graph
        .impact(
            &key("chain.py", "f12"),
            &ImpactOptions {
                max_depth: 20,
                max_entrypoints: 5,
            },
            &ModuleMap::default(),
        )
        .unwrap();
    assert_eq!(full.callers.len(), 12);
    assert!(!full.coverage.truncated);
}

#[test]
fn manifests_declare_entrypoints() {
    let graph = project(
        &[
            (
                "src/pkg/cli.py",
                "def main():\n    return helper()\n\ndef helper():\n    return 0\n",
            ),
            ("web/bin/tool.js", "const run = () => 1;\nrun();\n"),
            (
                "web/src/index.js",
                "export function api() { return inner(); }\nfunction inner() { return 1; }\n",
            ),
        ],
        &[
            (
                "pyproject.toml",
                "[project.scripts]\ntool = \"pkg.cli:main\"\n",
            ),
            (
                "web/package.json",
                r#"{"main":"src/index.js","bin":{"tool":"bin/tool.js"}}"#,
            ),
        ],
    );
    let helper = impact(&graph, "src/pkg/cli.py", "helper");
    let script = &helper.entrypoints[0];
    assert_eq!(script.entrypoint.kind, EntrypointKind::Cli);
    assert!(script.entrypoint.evidence.contains("[project.scripts]"));
    assert_eq!(script.entrypoint.scope, EntrypointScope::External);

    let inner = impact(&graph, "web/src/index.js", "inner");
    assert!(
        inner
            .entrypoints
            .iter()
            .any(|e| e.entrypoint.evidence.contains("package.json main")),
        "{:?}",
        inner.entrypoints
    );
    assert!(
        graph
            .entrypoints_at(&key("web/bin/tool.js", "<module>"))
            .iter()
            .any(|e| e.kind == EntrypointKind::Cli)
    );
}

#[test]
fn javascript_imports_barrels_and_express_routes() {
    let graph = project(
        &[
            (
                "server/routes.ts",
                "import express from 'express';\nimport { createUser } from './users';\nconst app = express();\napp.post('/users', (req, res) => { createUser(req.body); });\n",
            ),
            (
                "server/users/index.ts",
                "export { createUser } from './create';\n",
            ),
            (
                "server/users/create.ts",
                "import { hash } from '../crypto';\nexport function createUser(body) { return hash(body.password); }\n",
            ),
            (
                "server/crypto.ts",
                "export function hash(value: string) { return value; }\n",
            ),
        ],
        &[],
    );
    let hash = impact(&graph, "server/crypto.ts", "hash");
    assert_eq!(
        callers(&hash, 1),
        vec!["server/users/create.ts › createUser"]
    );
    let create = impact(&graph, "server/users/create.ts", "createUser");
    assert_eq!(
        callers(&create, 1),
        vec!["server/routes.ts › POST /users"],
        "the call goes through the re-exporting barrel"
    );
    assert!(create.callers[0].callers[0].origin <= EdgeOrigin::Heuristic);
    let route = hash
        .entrypoints
        .iter()
        .find(|e| matches!(e.entrypoint.kind, EntrypointKind::Http { .. }))
        .unwrap();
    assert_eq!(
        route.entrypoint.kind,
        EntrypointKind::Http {
            method: "POST".into(),
            route: "/users".into()
        }
    );
    assert_eq!(route.path.len(), 3);
}

#[test]
fn rust_and_go_calls_across_files() {
    let rust = project(
        &[
            (
                "src/lib.rs",
                "pub mod auth;\npub fn entry() { auth::check(); }\n",
            ),
            (
                "src/auth.rs",
                "pub fn check() { inner(); }\nfn inner() {}\n",
            ),
        ],
        &[],
    );
    let inner = impact(&rust, "src/auth.rs", "inner");
    assert_eq!(callers(&inner, 1), vec!["src/auth.rs › check"]);

    let go = project(
        &[
            (
                "cmd/app/main.go",
                "package main\n\nimport \"example.com/app/pkg/auth\"\n\nfunc main() {\n    auth.Check()\n}\n",
            ),
            (
                "pkg/auth/check.go",
                "package auth\n\nfunc Check() {\n    helper()\n}\n\nfunc helper() {}\n",
            ),
        ],
        &[],
    );
    let helper = impact(&go, "pkg/auth/check.go", "helper");
    assert_eq!(callers(&helper, 1), vec!["pkg/auth/check.go › Check"]);
    assert_eq!(callers(&helper, 2), vec!["cmd/app/main.go › main"]);
    assert!(
        helper
            .entrypoints
            .iter()
            .any(|e| e.entrypoint.kind == EntrypointKind::Main)
    );
}

#[test]
fn scope_distinguishes_local_module_and_cross_module() {
    let graph = project(
        &[
            (
                "src/core/a.py",
                "def leaf():\n    return 1\n\ndef inside():\n    return leaf()\n",
            ),
            (
                "src/core/b.py",
                "from .a import leaf\n\ndef sibling():\n    return leaf()\n",
            ),
            (
                "src/web/c.py",
                "from core.a import inside\n\ndef outer():\n    return inside()\n",
            ),
        ],
        &[],
    );
    let modules = ModuleMap::default();
    let scope = |file: &str, name: &str| {
        graph
            .impact(&key(file, name), &ImpactOptions::default(), &modules)
            .unwrap()
            .scope
    };
    assert_eq!(scope("src/web/c.py", "outer"), ImpactScope::Local);
    assert_eq!(
        scope("src/core/a.py", "inside"),
        ImpactScope::CrossModule,
        "called from src/web"
    );
    assert_eq!(
        scope("src/core/a.py", "leaf"),
        ImpactScope::CrossModule,
        "reached by inside <- outer"
    );
    let configured = ModuleMap::configured(&["src".into()]);
    let in_one_module = graph
        .impact(
            &key("src/core/a.py", "leaf"),
            &ImpactOptions::default(),
            &configured,
        )
        .unwrap();
    assert_eq!(
        in_one_module.scope,
        ImpactScope::Module,
        "with `src` as the only module everything is inside it"
    );
}

#[test]
fn module_cycles_come_from_file_imports() {
    let graph = project(
        &[
            (
                "src/api/routes.py",
                "from domain.rules import check\n\ndef route():\n    return check()\n",
            ),
            (
                "src/domain/rules.py",
                "from api.routes import route\n\ndef check():\n    return 1\n",
            ),
            ("src/infra/db.py", "from domain.rules import check\n"),
        ],
        &[],
    );
    assert_eq!(graph.import_edges().len(), 3);
    let cycles = graph.module_cycles(&ModuleMap::default());
    assert_eq!(
        cycles,
        vec![vec!["src/api".to_owned(), "src/domain".to_owned()]]
    );
}

#[test]
fn many_files_build_quickly() {
    let files: Vec<(String, String)> = (0..300)
        .map(|i| {
            (
                format!("pkg/m{i}.py"),
                format!(
                    "from pkg.m{} import f{}\n\ndef f{i}():\n    return f{}()\n",
                    (i + 1) % 300,
                    (i + 1) % 300,
                    (i + 1) % 300
                ),
            )
        })
        .collect();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    let started = std::time::Instant::now();
    let graph = project(&refs, &[]);
    assert_eq!(graph.call_edge_count(), 300);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
}
