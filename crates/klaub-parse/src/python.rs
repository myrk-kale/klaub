//! Python (tree-sitter-python).

use klaub_core::graph::{EdgeOrigin, EntrypointKind, EntrypointScope};
use klaub_core::language::Language;
use klaub_core::symbol::{
    EntrypointHint, HighlightClass, ImportRef, ImportedName, MarkerKind, Receiver, SymbolKind,
    Visibility,
};
use tree_sitter::Node;

use crate::engine::{
    CallParts, Def, EntryCtx, Profile, collapse, external, header, line_of, string_value, text,
};
use crate::markers::{
    dynamic_exec_call, io_call, plausible_secret, receiver_last, secret_name, shell_call, sql_sink,
};

pub(crate) struct Python;

const KEYWORDS: &[&str] = &[
    "def", "class", "return", "if", "elif", "else", "for", "while", "in", "not", "and", "or", "is",
    "import", "from", "as", "try", "except", "finally", "raise", "with", "pass", "break",
    "continue", "lambda", "yield", "async", "await", "global", "nonlocal", "assert", "del",
    "match", "case", "None", "True", "False",
];
const HTTP_METHODS: &[&str] = &[
    "get",
    "post",
    "put",
    "delete",
    "patch",
    "options",
    "head",
    "websocket",
    "route",
    "api_route",
];
const FRAMEWORK_CONSTRUCTORS: &[&str] = &[
    "FastAPI",
    "APIRouter",
    "Flask",
    "Blueprint",
    "Sanic",
    "Starlette",
    "Quart",
];

fn visibility(name: &str) -> Visibility {
    if name.starts_with('_') && !(name.starts_with("__") && name.ends_with("__")) {
        Visibility::Private
    } else {
        Visibility::Public
    }
}

fn dotted_path(node: Node<'_>, src: &[u8]) -> Option<Vec<String>> {
    match node.kind() {
        "identifier" => Some(vec![text(node, src).to_owned()]),
        "attribute" => {
            let mut path = dotted_path(node.child_by_field_name("object")?, src)?;
            path.push(text(node.child_by_field_name("attribute")?, src).to_owned());
            Some(path)
        }
        _ => None,
    }
}

fn is_dynamic_string(node: Node<'_>, src: &[u8]) -> bool {
    match node.kind() {
        "string" => {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .any(|c| c.kind() == "interpolation")
        }
        "binary_operator" => {
            let op = node
                .child_by_field_name("operator")
                .map_or("", |o| text(o, src));
            let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) else {
                return false;
            };
            match op {
                "%" => left.kind() == "string",
                "+" => {
                    let plain = |n: Node<'_>| n.kind() == "string" && !is_dynamic_string(n, src);
                    (left.kind() == "string" || right.kind() == "string")
                        && !(plain(left) && plain(right))
                }
                _ => false,
            }
        }
        "call" => node.child_by_field_name("function").is_some_and(|f| {
            f.kind() == "attribute"
                && f.child_by_field_name("attribute")
                    .is_some_and(|a| text(a, src) == "format")
                && f.child_by_field_name("object")
                    .is_some_and(|o| o.kind() == "string")
        }),
        _ => false,
    }
}

fn block_only_discards(block: Node<'_>) -> bool {
    let mut cursor = block.walk();
    let mut statements = block
        .named_children(&mut cursor)
        .filter(|c| c.kind() != "comment")
        .peekable();
    statements.peek().is_some()
        && statements.all(|s| {
            matches!(s.kind(), "pass_statement" | "continue_statement")
                || (s.kind() == "expression_statement"
                    && s.named_child(0).is_some_and(|e| e.kind() == "ellipsis"))
        })
}

impl Profile for Python {
    fn lang(&self) -> Language {
        Language::Python
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn definition<'t>(&self, node: Node<'t>, src: &'t [u8], parent: SymbolKind) -> Option<Def<'t>> {
        let (kind, name_field) = match node.kind() {
            "function_definition" => (
                if parent == SymbolKind::Class {
                    SymbolKind::Method
                } else {
                    SymbolKind::Function
                },
                "name",
            ),
            "class_definition" => (SymbolKind::Class, "name"),
            _ => return None,
        };
        let name = text(node.child_by_field_name(name_field)?, src).to_owned();
        let wrapper = node.parent().filter(|p| p.kind() == "decorated_definition");
        let decorators = wrapper
            .map(|w| {
                let mut cursor = w.walk();
                w.children(&mut cursor)
                    .filter(|c| c.kind() == "decorator")
                    .map(|d| collapse(text(d, src).trim_start_matches('@')))
                    .collect()
            })
            .unwrap_or_default();
        let is_async = node.child(0).is_some_and(|c| c.kind() == "async");
        let param_count = node.child_by_field_name("parameters").map_or(0, |p| {
            let mut cursor = p.walk();
            let count = p
                .named_children(&mut cursor)
                .filter(|c| {
                    !matches!(
                        c.kind(),
                        "keyword_separator" | "positional_separator" | "comment"
                    )
                })
                .count();
            let first_is_self = p
                .named_child(0)
                .is_some_and(|c| matches!(text(c, src), "self" | "cls"));
            u32::try_from(
                count.saturating_sub(usize::from(first_is_self && kind == SymbolKind::Method)),
            )
            .unwrap_or(u32::MAX)
        });
        Some(Def {
            kind,
            signature: header(node, src, 300),
            visibility: visibility(&name),
            is_test: (kind != SymbolKind::Class && name.starts_with("test"))
                || (kind == SymbolKind::Class && name.starts_with("Test")),
            name,
            decorators,
            is_async,
            param_count,
            node,
            span_node: wrapper.unwrap_or(node),
            child_prefix: None,
        })
    }

    fn is_comment(&self, kind: &str) -> bool {
        kind == "comment"
    }

    fn is_loop(&self, kind: &str) -> bool {
        matches!(
            kind,
            "for_statement"
                | "while_statement"
                | "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression"
        )
    }

    fn decision_points(&self, node: Node<'_>, _src: &[u8]) -> u32 {
        u32::from(matches!(
            node.kind(),
            "if_statement"
                | "elif_clause"
                | "for_statement"
                | "while_statement"
                | "except_clause"
                | "except_group_clause"
                | "case_clause"
                | "conditional_expression"
                | "for_in_clause"
                | "if_clause"
                | "boolean_operator"
        ))
    }

    fn is_nesting(&self, kind: &str) -> bool {
        matches!(
            kind,
            "if_statement"
                | "for_statement"
                | "while_statement"
                | "try_statement"
                | "with_statement"
                | "match_statement"
        )
    }

    fn call<'t>(&self, node: Node<'t>, src: &'t [u8]) -> Option<CallParts<'t>> {
        if node.kind() != "call" {
            return None;
        }
        let function = node.child_by_field_name("function")?;
        let (name, receiver) = match function.kind() {
            "identifier" => (text(function, src).to_owned(), Receiver::None),
            "attribute" => {
                let name = text(function.child_by_field_name("attribute")?, src).to_owned();
                let object = function.child_by_field_name("object")?;
                let receiver = match dotted_path(object, src) {
                    Some(path) if path.len() == 1 && matches!(path[0].as_str(), "self" | "cls") => {
                        Receiver::SelfRef
                    }
                    Some(path) => Receiver::Path(path),
                    None => Receiver::Complex,
                };
                (name, receiver)
            }
            _ => return None,
        };
        let mut args = Vec::new();
        let mut kwargs = Vec::new();
        if let Some(list) = node.child_by_field_name("arguments") {
            let mut cursor = list.walk();
            for arg in list.named_children(&mut cursor) {
                if arg.kind() == "keyword_argument" {
                    if let (Some(key), Some(value)) = (
                        arg.child_by_field_name("name"),
                        arg.child_by_field_name("value"),
                    ) {
                        kwargs.push((text(key, src).to_owned(), value));
                    }
                } else if arg.kind() != "comment" {
                    args.push(arg);
                }
            }
        }
        Some(CallParts {
            name,
            receiver,
            args,
            kwargs,
        })
    }

    fn imports(&self, node: Node<'_>, src: &[u8]) -> Vec<ImportRef> {
        let line = line_of(node);
        let item = |n: Node<'_>| -> (String, Option<String>) {
            if n.kind() == "aliased_import" {
                (
                    n.child_by_field_name("name")
                        .map_or(String::new(), |x| text(x, src).to_owned()),
                    n.child_by_field_name("alias")
                        .map(|a| text(a, src).to_owned()),
                )
            } else {
                (text(n, src).to_owned(), None)
            }
        };
        match node.kind() {
            "import_statement" => {
                let mut cursor = node.walk();
                node.children_by_field_name("name", &mut cursor)
                    .map(|n| {
                        let (module, alias) = item(n);
                        ImportRef {
                            module,
                            relative_level: 0,
                            names: vec![],
                            alias,
                            wildcard: false,
                            reexport: false,
                            dynamic: false,
                            line,
                        }
                    })
                    .collect()
            }
            "import_from_statement" => {
                let module_node = node.child_by_field_name("module_name");
                let raw = module_node.map_or("", |m| text(m, src));
                let level = raw.chars().take_while(|c| *c == '.').count();
                let module = raw.trim_start_matches('.').to_owned();
                let mut cursor = node.walk();
                let names = node
                    .children_by_field_name("name", &mut cursor)
                    .map(|n| {
                        let (name, alias) = item(n);
                        ImportedName { name, alias }
                    })
                    .collect();
                let mut cursor = node.walk();
                let wildcard = node
                    .children(&mut cursor)
                    .any(|c| c.kind() == "wildcard_import");
                vec![ImportRef {
                    module,
                    relative_level: u32::try_from(level).unwrap_or(u32::MAX),
                    names,
                    alias: None,
                    wildcard,
                    reexport: false,
                    dynamic: false,
                    line,
                }]
            }
            _ => vec![],
        }
    }

    fn node_markers(&self, node: Node<'_>, src: &[u8]) -> Vec<(MarkerKind, String)> {
        match node.kind() {
            "except_clause" | "except_group_clause" => {
                let mut cursor = node.walk();
                let body = node.children(&mut cursor).find(|c| c.kind() == "block");
                if body.is_some_and(block_only_discards) {
                    return vec![(
                        MarkerKind::SwallowedError,
                        "an except block discards the error".to_owned(),
                    )];
                }
                vec![]
            }
            "assignment"
            | "keyword_argument"
            | "default_parameter"
            | "typed_default_parameter"
            | "pair" => {
                let (name_node, value) = match node.kind() {
                    "assignment" => (
                        node.child_by_field_name("left"),
                        node.child_by_field_name("right"),
                    ),
                    "pair" => (
                        node.child_by_field_name("key"),
                        node.child_by_field_name("value"),
                    ),
                    _ => (
                        node.child_by_field_name("name"),
                        node.child_by_field_name("value"),
                    ),
                };
                let (Some(name_node), Some(value)) = (name_node, value) else {
                    return vec![];
                };
                let name = match name_node.kind() {
                    "identifier" => text(name_node, src).to_owned(),
                    "attribute" => name_node
                        .child_by_field_name("attribute")
                        .map_or(String::new(), |a| text(a, src).to_owned()),
                    "string" => string_value(name_node, src).unwrap_or_default(),
                    _ => return vec![],
                };
                if value.kind() == "string"
                    && !is_dynamic_string(value, src)
                    && secret_name(&name)
                    && string_value(value, src).is_some_and(|v| plausible_secret(&v))
                {
                    return vec![(
                        MarkerKind::SecretLiteral,
                        format!("a string literal is assigned to `{name}`"),
                    )];
                }
                vec![]
            }
            _ => vec![],
        }
    }

    fn call_markers(
        &self,
        call: &CallParts<'_>,
        in_loop: bool,
        src: &[u8],
    ) -> Vec<(MarkerKind, String)> {
        let mut found = Vec::new();
        if sql_sink(&call.name)
            && call
                .args
                .first()
                .is_some_and(|a| is_dynamic_string(*a, src))
        {
            found.push((
                MarkerKind::RawSqlConcat,
                format!("`{}` receives a query built from dynamic text", call.name),
            ));
        }
        if dynamic_exec_call(&call.receiver, &call.name) {
            found.push((
                MarkerKind::DynamicExecution,
                format!("`{}` evaluates text as code", call.name),
            ));
        }
        let shell_true = call
            .kwargs
            .iter()
            .any(|(k, v)| k == "shell" && v.kind() == "true");
        if shell_call(&call.receiver, &call.name)
            || (shell_true && receiver_last(&call.receiver) == Some("subprocess"))
        {
            found.push((
                MarkerKind::ShellExecution,
                format!("`{}` runs a shell command", call.name),
            ));
        }
        if call
            .kwargs
            .iter()
            .any(|(k, v)| matches!(k.as_str(), "verify" | "check_hostname") && v.kind() == "false")
            || call.name == "_create_unverified_context"
        {
            found.push((
                MarkerKind::VerificationDisabled,
                "certificate verification is turned off".to_owned(),
            ));
        }
        if in_loop && io_call(&call.receiver, &call.name) {
            found.push((
                MarkerKind::CallInLoop,
                format!("`{}` performs I/O inside a loop", call.name),
            ));
        }
        found
    }

    fn entrypoints(&self, ctx: &EntryCtx<'_, '_>) -> Vec<EntrypointHint> {
        let mut hints = Vec::new();
        let mut cursor = ctx.root.walk();
        let mut framework_objects: Vec<String> = Vec::new();
        for statement in ctx.root.children(&mut cursor) {
            if statement.kind() == "if_statement" {
                let condition = statement
                    .child_by_field_name("condition")
                    .map_or("", |c| text(c, ctx.src));
                if condition.contains("__name__") && condition.contains("__main__") {
                    hints.push(EntrypointHint {
                        symbol: Some(0),
                        scope: EntrypointScope::External,
                        kind: EntrypointKind::Cli,
                        origin: EdgeOrigin::Resolved,
                        evidence: "if __name__ == \"__main__\"".to_owned(),
                        line: line_of(statement),
                    });
                }
            }
            if statement.kind() == "expression_statement"
                && let Some(assignment) = statement
                    .named_child(0)
                    .filter(|n| n.kind() == "assignment")
                && let (Some(left), Some(right)) = (
                    assignment.child_by_field_name("left"),
                    assignment.child_by_field_name("right"),
                )
                && right.kind() == "call"
                && let Some(function) = right.child_by_field_name("function")
            {
                let callee = text(function, ctx.src);
                if FRAMEWORK_CONSTRUCTORS
                    .iter()
                    .any(|c| callee == *c || callee.ends_with(&format!(".{c}")))
                {
                    framework_objects.push(text(left, ctx.src).to_owned());
                }
            }
        }

        for (index, symbol) in ctx.symbols.iter().enumerate() {
            for decorator in &symbol.decorators {
                let Some((callee, rest)) = decorator.split_once('(') else {
                    continue;
                };
                let Some((object, method)) = callee.rsplit_once('.') else {
                    continue;
                };
                if !HTTP_METHODS.contains(&method) {
                    continue;
                }
                let route = rest.split(['"', '\'']).nth(1).unwrap_or("").to_owned();
                let http_method = if matches!(method, "route" | "api_route") {
                    rest.split("methods")
                        .nth(1)
                        .and_then(|m| m.split(['"', '\'']).nth(1))
                        .map_or_else(|| "ANY".to_owned(), str::to_ascii_uppercase)
                } else {
                    method.to_ascii_uppercase()
                };
                let origin = if framework_objects.iter().any(|o| o == object) {
                    EdgeOrigin::Resolved
                } else {
                    EdgeOrigin::Heuristic
                };
                hints.push(external(
                    origin,
                    Some(index),
                    EntrypointKind::Http {
                        method: http_method,
                        route,
                    },
                    format!("decorator @{}", collapse(decorator)),
                    symbol.span.start_line,
                ));
            }
        }
        hints
    }

    fn highlight(&self, node: Node<'_>, src: &[u8]) -> Option<HighlightClass> {
        match node.kind() {
            "comment" => Some(HighlightClass::Comment),
            "string" | "concatenated_string" => Some(HighlightClass::String),
            "integer" | "float" => Some(HighlightClass::Number),
            "identifier" => {
                let parent = node.parent()?;
                let is_name = parent
                    .child_by_field_name("name")
                    .is_some_and(|n| n.id() == node.id());
                match (parent.kind(), is_name) {
                    ("function_definition", true) => Some(HighlightClass::Function),
                    ("class_definition", true) => Some(HighlightClass::Type),
                    _ => None,
                }
            }
            kind if !node.is_named() && KEYWORDS.contains(&kind) && text(node, src) == kind => {
                Some(HighlightClass::Keyword)
            }
            "true" | "false" | "none" => Some(HighlightClass::Keyword),
            _ => None,
        }
    }
}
