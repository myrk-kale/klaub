//! JavaScript, TypeScript and TSX (tree-sitter-javascript / tree-sitter-typescript).

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
    dynamic_exec_call, io_call, plausible_secret, secret_name, shell_call, sql_sink,
};

pub(crate) struct JavaScript {
    pub(crate) language: Language,
}

const KEYWORDS: &[&str] = &[
    "function",
    "class",
    "const",
    "let",
    "var",
    "return",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "try",
    "catch",
    "finally",
    "throw",
    "new",
    "delete",
    "typeof",
    "instanceof",
    "in",
    "of",
    "import",
    "export",
    "from",
    "as",
    "async",
    "await",
    "yield",
    "extends",
    "static",
    "get",
    "set",
    "interface",
    "type",
    "enum",
    "implements",
    "public",
    "private",
    "protected",
    "readonly",
    "abstract",
    "declare",
    "namespace",
    "this",
    "super",
    "void",
];
const ROUTE_METHODS: &[&str] = &[
    "get", "post", "put", "delete", "patch", "options", "head", "all",
];
const FRAMEWORK_CONSTRUCTORS: &[&str] = &[
    "express",
    "express.Router",
    "Router",
    "fastify",
    "Fastify",
    "Hono",
    "Koa",
];

fn dotted_path(node: Node<'_>, src: &[u8]) -> Option<Vec<String>> {
    match node.kind() {
        "identifier" | "property_identifier" => Some(vec![text(node, src).to_owned()]),
        "this" => Some(vec!["this".to_owned()]),
        "member_expression" => {
            let mut path = dotted_path(node.child_by_field_name("object")?, src)?;
            path.push(text(node.child_by_field_name("property")?, src).to_owned());
            Some(path)
        }
        _ => None,
    }
}

fn is_dynamic_string(node: Node<'_>) -> bool {
    match node.kind() {
        "template_string" => {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .any(|c| c.kind() == "template_substitution")
        }
        "binary_expression" => {
            let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) else {
                return false;
            };
            let plain = |n: Node<'_>| n.kind() == "string";
            (plain(left) || plain(right)) && !(plain(left) && plain(right))
                || (left.kind() == "binary_expression" && is_dynamic_string(left))
        }
        _ => false,
    }
}

fn route_of(call: Node<'_>, src: &[u8]) -> Option<(String, String)> {
    if call.kind() != "call_expression" {
        return None;
    }
    let function = call
        .child_by_field_name("function")
        .filter(|f| f.kind() == "member_expression")?;
    let method = text(function.child_by_field_name("property")?, src);
    if !ROUTE_METHODS.contains(&method) {
        return None;
    }
    let arguments = call.child_by_field_name("arguments")?;
    let first = arguments.named_child(0).filter(|n| n.kind() == "string")?;
    let route = string_value(first, src)?;
    (route.starts_with('/') || route == "*").then(|| (method.to_ascii_uppercase(), route))
}

fn is_function_value(kind: &str) -> bool {
    matches!(
        kind,
        "arrow_function" | "function_expression" | "function" | "generator_function"
    )
}

fn has_child(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| c.kind() == kind)
}

fn param_count(node: Node<'_>) -> u32 {
    if let Some(params) = node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        return u32::try_from(
            params
                .named_children(&mut cursor)
                .filter(|c| c.kind() != "comment")
                .count(),
        )
        .unwrap_or(u32::MAX);
    }
    u32::from(node.child_by_field_name("parameter").is_some())
}

impl Profile for JavaScript {
    fn lang(&self) -> Language {
        self.language
    }

    fn ts_language(&self) -> tree_sitter::Language {
        match self.language {
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => tree_sitter_javascript::LANGUAGE.into(),
        }
    }

    fn definition<'t>(
        &self,
        node: Node<'t>,
        src: &'t [u8],
        _parent: SymbolKind,
    ) -> Option<Def<'t>> {
        let kind_name = node.kind();
        let parent = node.parent();
        let exported_via = |n: Node<'t>| n.parent().filter(|p| p.kind() == "export_statement");

        // Inline callbacks of route registrations are symbols of their own.
        if is_function_value(kind_name)
            && let Some(arguments) = parent.filter(|p| p.kind() == "arguments")
            && let Some((method, route)) = arguments.parent().and_then(|call| route_of(call, src))
        {
            return Some(Def {
                kind: SymbolKind::Function,
                name: format!("{method} {route}"),
                signature: format!("{method} {route} handler"),
                visibility: Visibility::Private,
                decorators: vec![],
                is_async: has_child(node, "async"),
                is_test: false,
                param_count: param_count(node),
                node,
                span_node: node,
                child_prefix: None,
            });
        }

        let (kind, name_node, span_node, value) = match kind_name {
            "function_declaration" | "generator_function_declaration" => (
                SymbolKind::Function,
                node.child_by_field_name("name")?,
                exported_via(node).unwrap_or(node),
                None,
            ),
            "class_declaration" | "abstract_class_declaration" => (
                SymbolKind::Class,
                node.child_by_field_name("name")?,
                exported_via(node).unwrap_or(node),
                None,
            ),
            "interface_declaration" => (
                SymbolKind::Interface,
                node.child_by_field_name("name")?,
                exported_via(node).unwrap_or(node),
                None,
            ),
            "type_alias_declaration" => (
                SymbolKind::TypeAlias,
                node.child_by_field_name("name")?,
                exported_via(node).unwrap_or(node),
                None,
            ),
            "enum_declaration" => (
                SymbolKind::Enum,
                node.child_by_field_name("name")?,
                exported_via(node).unwrap_or(node),
                None,
            ),
            "method_definition" => (
                SymbolKind::Method,
                node.child_by_field_name("name")?,
                node,
                None,
            ),
            "variable_declarator" => {
                let name = node
                    .child_by_field_name("name")
                    .filter(|n| n.kind() == "identifier")?;
                let value = node
                    .child_by_field_name("value")
                    .filter(|v| is_function_value(v.kind()))?;
                let declaration = parent
                    .filter(|p| matches!(p.kind(), "lexical_declaration" | "variable_declaration"));
                let single = declaration.is_some_and(|d| {
                    let mut cursor = d.walk();
                    d.children(&mut cursor)
                        .filter(|c| c.kind() == "variable_declarator")
                        .count()
                        == 1
                });
                let outer = match (declaration, single) {
                    (Some(d), true) => exported_via(d).unwrap_or(d),
                    _ => node,
                };
                (SymbolKind::Function, name, outer, Some(value))
            }
            "public_field_definition" | "field_definition" => {
                let name = node
                    .child_by_field_name("name")
                    .or_else(|| node.child_by_field_name("property"))?;
                let value = node
                    .child_by_field_name("value")
                    .filter(|v| is_function_value(v.kind()))?;
                (SymbolKind::Method, name, node, Some(value))
            }
            _ => return None,
        };

        let name = text(name_node, src).trim_matches(['"', '\'']).to_owned();
        let exported = exported_via(span_node).is_some() || span_node.kind() == "export_statement";
        let modifier = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "accessibility_modifier")
                .map(|m| text(m, src).to_owned())
        };
        let visibility = match (kind, modifier.as_deref()) {
            (_, Some("private")) => Visibility::Private,
            (_, Some("protected")) => Visibility::Protected,
            (SymbolKind::Method, _) if name.starts_with('#') => Visibility::Private,
            (SymbolKind::Method, _) => Visibility::Public,
            _ if exported => Visibility::Public,
            _ => Visibility::Private,
        };
        let signature = match value {
            Some(function) => {
                let end = function
                    .child_by_field_name("body")
                    .map_or(function.end_byte(), |b| b.start_byte());
                let start = span_node
                    .start_byte()
                    .max(if span_node.kind() == "export_statement" {
                        node.start_byte()
                    } else {
                        0
                    });
                let raw = src
                    .get(start..end)
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .unwrap_or("");
                collapse(raw).trim_end().to_owned()
            }
            None => header(node, src, 300),
        };
        let decorators = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .filter(|c| c.kind() == "decorator")
                .map(|d| collapse(text(d, src).trim_start_matches('@')))
                .collect()
        };
        let function_node = value.unwrap_or(node);
        Some(Def {
            kind,
            signature,
            visibility,
            is_async: has_child(function_node, "async"),
            is_test: kind == SymbolKind::Function && name.starts_with("test"),
            param_count: param_count(function_node),
            name,
            decorators,
            node,
            span_node,
            child_prefix: None,
        })
    }

    fn is_comment(&self, kind: &str) -> bool {
        kind == "comment"
    }

    fn is_loop(&self, kind: &str) -> bool {
        matches!(
            kind,
            "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
        )
    }

    fn decision_points(&self, node: Node<'_>, src: &[u8]) -> u32 {
        match node.kind() {
            "if_statement" | "for_statement" | "for_in_statement" | "while_statement"
            | "do_statement" | "switch_case" | "catch_clause" | "ternary_expression" => 1,
            "binary_expression" => u32::from(
                node.child_by_field_name("operator")
                    .is_some_and(|o| matches!(text(o, src), "&&" | "||" | "??")),
            ),
            _ => 0,
        }
    }

    fn is_nesting(&self, kind: &str) -> bool {
        matches!(
            kind,
            "if_statement"
                | "for_statement"
                | "for_in_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
                | "try_statement"
        )
    }

    fn call<'t>(&self, node: Node<'t>, src: &'t [u8]) -> Option<CallParts<'t>> {
        let function = match node.kind() {
            "call_expression" => node.child_by_field_name("function")?,
            "new_expression" => node.child_by_field_name("constructor")?,
            _ => return None,
        };
        let (name, receiver) = match function.kind() {
            "identifier" => (text(function, src).to_owned(), Receiver::None),
            "member_expression" => {
                let name = text(function.child_by_field_name("property")?, src).to_owned();
                let object = function.child_by_field_name("object")?;
                let receiver = match dotted_path(object, src) {
                    Some(path) if path.len() == 1 && path[0] == "this" => Receiver::SelfRef,
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
                if arg.kind() == "comment" {
                    continue;
                }
                if arg.kind() == "object" {
                    let mut inner = arg.walk();
                    for pair in arg
                        .named_children(&mut inner)
                        .filter(|c| c.kind() == "pair")
                    {
                        if let (Some(key), Some(value)) = (
                            pair.child_by_field_name("key"),
                            pair.child_by_field_name("value"),
                        ) {
                            kwargs
                                .push((text(key, src).trim_matches(['"', '\'']).to_owned(), value));
                        }
                    }
                }
                args.push(arg);
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
        let named = |clause: Node<'_>| -> Vec<ImportedName> {
            let mut cursor = clause.walk();
            clause
                .named_children(&mut cursor)
                .filter(|c| matches!(c.kind(), "import_specifier" | "export_specifier"))
                .filter_map(|spec| {
                    let name = text(spec.child_by_field_name("name")?, src).to_owned();
                    Some(ImportedName {
                        name,
                        alias: spec
                            .child_by_field_name("alias")
                            .map(|a| text(a, src).to_owned()),
                    })
                })
                .collect()
        };
        match node.kind() {
            "import_statement" | "export_statement" => {
                let reexport = node.kind() == "export_statement";
                let Some(module) = node
                    .child_by_field_name("source")
                    .and_then(|s| string_value(s, src))
                else {
                    return vec![];
                };
                let mut names = Vec::new();
                let mut alias = None;
                let mut wildcard = false;
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "import_clause" => {
                            let mut inner = child.walk();
                            for part in child.named_children(&mut inner) {
                                match part.kind() {
                                    "identifier" => names.push(ImportedName {
                                        name: "default".to_owned(),
                                        alias: Some(text(part, src).to_owned()),
                                    }),
                                    "namespace_import" => {
                                        wildcard = true;
                                        alias =
                                            part.named_child(0).map(|a| text(a, src).to_owned());
                                    }
                                    "named_imports" => names.extend(named(part)),
                                    _ => {}
                                }
                            }
                        }
                        "export_clause" => names.extend(named(child)),
                        "namespace_export" => {
                            wildcard = true;
                            alias = child.named_child(0).map(|a| text(a, src).to_owned());
                        }
                        "*" => wildcard = true,
                        _ => {}
                    }
                }
                vec![ImportRef {
                    module,
                    relative_level: 0,
                    names,
                    alias,
                    wildcard,
                    reexport,
                    dynamic: false,
                    line,
                }]
            }
            "call_expression" => {
                let Some(function) = node.child_by_field_name("function") else {
                    return vec![];
                };
                let is_require =
                    function.kind() == "identifier" && text(function, src) == "require";
                if !(is_require || function.kind() == "import") {
                    return vec![];
                }
                let Some(module) = node
                    .child_by_field_name("arguments")
                    .and_then(|a| a.named_child(0))
                    .and_then(|s| string_value(s, src))
                else {
                    return vec![];
                };
                let mut alias = None;
                let mut names = Vec::new();
                if let Some(declarator) =
                    node.parent().filter(|p| p.kind() == "variable_declarator")
                    && let Some(target) = declarator.child_by_field_name("name")
                {
                    match target.kind() {
                        "identifier" => alias = Some(text(target, src).to_owned()),
                        "object_pattern" => {
                            let mut cursor = target.walk();
                            for part in target.named_children(&mut cursor) {
                                match part.kind() {
                                    "shorthand_property_identifier_pattern" => {
                                        names.push(ImportedName {
                                            name: text(part, src).to_owned(),
                                            alias: None,
                                        });
                                    }
                                    "pair_pattern" => {
                                        if let (Some(key), Some(value)) = (
                                            part.child_by_field_name("key"),
                                            part.child_by_field_name("value"),
                                        ) {
                                            names.push(ImportedName {
                                                name: text(key, src).to_owned(),
                                                alias: Some(text(value, src).to_owned()),
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                vec![ImportRef {
                    module,
                    relative_level: 0,
                    names,
                    alias,
                    wildcard: false,
                    reexport: false,
                    dynamic: true,
                    line,
                }]
            }
            _ => vec![],
        }
    }

    fn node_markers(&self, node: Node<'_>, src: &[u8]) -> Vec<(MarkerKind, String)> {
        match node.kind() {
            "catch_clause" => {
                let empty = node.child_by_field_name("body").is_some_and(|body| {
                    let mut cursor = body.walk();
                    body.named_children(&mut cursor)
                        .all(|c| c.kind() == "comment")
                });
                if empty {
                    vec![(
                        MarkerKind::SwallowedError,
                        "a catch block discards the error".to_owned(),
                    )]
                } else {
                    vec![]
                }
            }
            "variable_declarator"
            | "assignment_expression"
            | "pair"
            | "public_field_definition"
            | "field_definition" => {
                let (name_node, value) = match node.kind() {
                    "variable_declarator" | "public_field_definition" | "field_definition" => (
                        node.child_by_field_name("name")
                            .or_else(|| node.child_by_field_name("property")),
                        node.child_by_field_name("value"),
                    ),
                    "assignment_expression" => (
                        node.child_by_field_name("left"),
                        node.child_by_field_name("right"),
                    ),
                    _ => (
                        node.child_by_field_name("key"),
                        node.child_by_field_name("value"),
                    ),
                };
                let (Some(name_node), Some(value)) = (name_node, value) else {
                    return vec![];
                };
                let name = match name_node.kind() {
                    "member_expression" => name_node
                        .child_by_field_name("property")
                        .map_or(String::new(), |p| text(p, src).to_owned()),
                    "subscript_expression" => string_value(
                        name_node.child_by_field_name("index").unwrap_or(name_node),
                        src,
                    )
                    .unwrap_or_default(),
                    _ => text(name_node, src).trim_matches(['"', '\'']).to_owned(),
                };
                let mut found = Vec::new();
                if value.kind() == "string"
                    && secret_name(&name)
                    && string_value(value, src).is_some_and(|v| plausible_secret(&v))
                {
                    found.push((
                        MarkerKind::SecretLiteral,
                        format!("a string literal is assigned to `{name}`"),
                    ));
                }
                if node.kind() == "assignment_expression"
                    && text(name_node, src).contains("NODE_TLS_REJECT_UNAUTHORIZED")
                    && string_value(value, src).as_deref() == Some("0")
                {
                    found.push((
                        MarkerKind::VerificationDisabled,
                        "TLS verification is turned off for the process".to_owned(),
                    ));
                }
                found
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
        if sql_sink(&call.name) && call.args.first().is_some_and(|a| is_dynamic_string(*a)) {
            found.push((
                MarkerKind::RawSqlConcat,
                format!("`{}` receives a query built from dynamic text", call.name),
            ));
        }
        if dynamic_exec_call(&call.receiver, &call.name)
            || (matches!(call.receiver, Receiver::None) && call.name == "Function")
        {
            found.push((
                MarkerKind::DynamicExecution,
                format!("`{}` evaluates text as code", call.name),
            ));
        }
        if shell_call(&call.receiver, &call.name) {
            found.push((
                MarkerKind::ShellExecution,
                format!("`{}` runs a shell command", call.name),
            ));
        }
        if call.kwargs.iter().any(|(k, v)| {
            matches!(k.as_str(), "rejectUnauthorized" | "strictSSL") && text(*v, src) == "false"
        }) {
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

    fn entrypoints<'t>(&self, ctx: &EntryCtx<'_, 't>) -> Vec<EntrypointHint> {
        let mut hints = Vec::new();
        let mut frameworks: Vec<String> = Vec::new();
        let mut routes: Vec<(Node<'t>, String, String)> = Vec::new();

        let mut stack = vec![ctx.root];
        while let Some(node) = stack.pop() {
            if node.kind() == "variable_declarator"
                && let (Some(name), Some(value)) = (
                    node.child_by_field_name("name"),
                    node.child_by_field_name("value"),
                )
                && matches!(value.kind(), "call_expression" | "new_expression")
                && let Some(function) = value
                    .child_by_field_name("function")
                    .or_else(|| value.child_by_field_name("constructor"))
                && FRAMEWORK_CONSTRUCTORS.contains(&text(function, ctx.src))
            {
                frameworks.push(text(name, ctx.src).to_owned());
            }
            if let Some((method, route)) = route_of(node, ctx.src) {
                routes.push((node, method, route));
            }
            let mut cursor = node.walk();
            stack.extend(node.children(&mut cursor));
        }

        for (call, method, route) in routes {
            let Some(arguments) = call.child_by_field_name("arguments") else {
                continue;
            };
            let count = u32::try_from(arguments.named_child_count()).unwrap_or(0);
            let Some(handler) = count
                .checked_sub(1)
                .and_then(|i| arguments.named_child(i))
                .filter(|_| count >= 2)
            else {
                continue;
            };
            let symbol = if handler.kind() == "identifier" {
                let name = text(handler, ctx.src);
                ctx.symbols
                    .iter()
                    .position(|s| s.name == name && s.parent == Some(0))
            } else {
                ctx.nodes.iter().position(|n| n.id() == handler.id())
            };
            let object = call
                .child_by_field_name("function")
                .and_then(|f| f.child_by_field_name("object"))
                .map_or("", |o| text(o, ctx.src));
            let origin = if frameworks.iter().any(|f| f == object) {
                EdgeOrigin::Resolved
            } else {
                EdgeOrigin::Heuristic
            };
            hints.push(external(
                origin,
                symbol,
                EntrypointKind::Http {
                    method: method.clone(),
                    route: route.clone(),
                },
                format!("{object}.{}('{route}', ...)", method.to_ascii_lowercase()),
                line_of(call),
            ));
        }

        for (index, symbol) in ctx.symbols.iter().enumerate() {
            if symbol.parent == Some(0)
                && symbol.visibility == Visibility::Public
                && matches!(symbol.kind, SymbolKind::Function | SymbolKind::Class)
            {
                hints.push(EntrypointHint {
                    symbol: Some(index),
                    scope: EntrypointScope::Internal,
                    kind: EntrypointKind::PublicExport,
                    origin: EdgeOrigin::Resolved,
                    evidence: "exported from the module".to_owned(),
                    line: symbol.span.start_line,
                });
            }
        }
        hints
    }

    fn highlight(&self, node: Node<'_>, src: &[u8]) -> Option<HighlightClass> {
        match node.kind() {
            "comment" => Some(HighlightClass::Comment),
            "string" | "template_string" | "regex" => Some(HighlightClass::String),
            "number" => Some(HighlightClass::Number),
            "type_identifier" | "predefined_type" => Some(HighlightClass::Type),
            "true" | "false" | "null" | "undefined" => Some(HighlightClass::Keyword),
            "identifier" | "property_identifier" => {
                let parent = node.parent()?;
                let is_name = parent
                    .child_by_field_name("name")
                    .is_some_and(|n| n.id() == node.id());
                match (parent.kind(), is_name) {
                    ("function_declaration" | "method_definition", true) => {
                        Some(HighlightClass::Function)
                    }
                    (
                        "class_declaration"
                        | "interface_declaration"
                        | "enum_declaration"
                        | "type_alias_declaration",
                        true,
                    ) => Some(HighlightClass::Type),
                    _ => None,
                }
            }
            kind if !node.is_named() && KEYWORDS.contains(&kind) && text(node, src) == kind => {
                Some(HighlightClass::Keyword)
            }
            _ => None,
        }
    }
}
