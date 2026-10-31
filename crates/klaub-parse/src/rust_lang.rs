//! Rust (tree-sitter-rust).

use klaub_core::graph::{EdgeOrigin, EntrypointKind, EntrypointScope};
use klaub_core::language::Language;
use klaub_core::symbol::{
    EntrypointHint, HighlightClass, ImportRef, ImportedName, MarkerKind, Receiver, SymbolKind,
    Visibility,
};
use tree_sitter::Node;

use crate::engine::{
    CallParts, Def, EntryCtx, Profile, collapse, header, line_of, string_value, text,
};
use crate::markers::{io_call, plausible_secret, secret_name, sql_sink};

pub(crate) struct RustLang;

const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "struct", "enum", "trait", "impl", "for", "while", "loop", "if",
    "else", "match", "return", "use", "mod", "const", "static", "type", "where", "async", "await",
    "move", "unsafe", "dyn", "ref", "as", "in", "break", "continue", "self", "Self", "crate",
    "super", "extern",
];
const SHELLS: &[&str] = &["sh", "bash", "zsh", "cmd", "powershell", "pwsh"];

fn split_path(path: &str) -> Vec<String> {
    path.split("::")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn attributes(node: Node<'_>, src: &[u8]) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut current = node.prev_sibling();
    while let Some(sibling) = current {
        match sibling.kind() {
            "attribute_item" => {
                let raw = text(sibling, src);
                attrs.push(collapse(raw.trim_start_matches("#[").trim_end_matches(']')));
            }
            "line_comment" | "block_comment" => {}
            _ => break,
        }
        current = sibling.prev_sibling();
    }
    attrs.reverse();
    attrs
}

fn callee(function: Node<'_>, src: &[u8]) -> Option<(String, Receiver)> {
    match function.kind() {
        "identifier" => Some((text(function, src).to_owned(), Receiver::None)),
        "scoped_identifier" => {
            let name = text(function.child_by_field_name("name")?, src).to_owned();
            let path = function
                .child_by_field_name("path")
                .map_or_else(Vec::new, |p| split_path(text(p, src)));
            Some((
                name,
                if path.is_empty() {
                    Receiver::None
                } else {
                    Receiver::Path(path)
                },
            ))
        }
        "field_expression" => {
            let name = text(function.child_by_field_name("field")?, src).to_owned();
            let value = function.child_by_field_name("value")?;
            let receiver = match value.kind() {
                "self" => Receiver::SelfRef,
                "identifier" => Receiver::Path(vec![text(value, src).to_owned()]),
                "field_expression" => {
                    let flat: Vec<String> = text(value, src)
                        .split('.')
                        .map(|s| s.trim().to_owned())
                        .collect();
                    if flat
                        .iter()
                        .all(|s| s.chars().all(|c| c.is_alphanumeric() || c == '_'))
                    {
                        Receiver::Path(flat)
                    } else {
                        Receiver::Complex
                    }
                }
                _ => Receiver::Complex,
            };
            Some((name, receiver))
        }
        "generic_function" => callee(function.child_by_field_name("function")?, src),
        _ => None,
    }
}

fn flatten_use(node: Node<'_>, src: &[u8], prefix: &str, line: u32, out: &mut Vec<ImportRef>) {
    let join = |left: &str, right: &str| {
        if left.is_empty() {
            right.to_owned()
        } else if right.is_empty() {
            left.to_owned()
        } else {
            format!("{left}::{right}")
        }
    };
    let push = |out: &mut Vec<ImportRef>,
                module: String,
                name: Option<(String, Option<String>)>,
                wildcard: bool| {
        out.push(ImportRef {
            module,
            relative_level: 0,
            names: name
                .map(|(name, alias)| ImportedName { name, alias })
                .into_iter()
                .collect(),
            alias: None,
            wildcard,
            reexport: false,
            dynamic: false,
            line,
        });
    };
    match node.kind() {
        "identifier" | "scoped_identifier" | "crate" | "self" | "super" => {
            let full = join(prefix, text(node, src));
            match full.rsplit_once("::") {
                Some((module, name)) => {
                    push(out, module.to_owned(), Some((name.to_owned(), None)), false);
                }
                None => push(out, full, None, false),
            }
        }
        "use_as_clause" => {
            let (Some(path), alias) = (
                node.child_by_field_name("path"),
                node.child_by_field_name("alias"),
            ) else {
                return;
            };
            let full = join(prefix, text(path, src));
            let alias = alias.map(|a| text(a, src).to_owned());
            match full.rsplit_once("::") {
                Some((module, name)) => push(
                    out,
                    module.to_owned(),
                    Some((name.to_owned(), alias)),
                    false,
                ),
                None => push(out, full, None, false),
            }
        }
        "scoped_use_list" => {
            let path = node
                .child_by_field_name("path")
                .map_or("", |p| text(p, src));
            let new_prefix = join(prefix, path);
            if let Some(list) = node.child_by_field_name("list") {
                flatten_use(list, src, &new_prefix, line, out);
            }
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                flatten_use(child, src, prefix, line, out);
            }
        }
        "use_wildcard" => {
            let path = text(node, src).trim_end_matches('*').trim_end_matches("::");
            push(out, join(prefix, path), None, true);
        }
        _ => {}
    }
}

fn is_trait_impl_member(node: Node<'_>) -> bool {
    node.parent()
        .and_then(|list| list.parent())
        .is_some_and(|owner| {
            owner.kind() == "trait_item"
                || (owner.kind() == "impl_item" && owner.child_by_field_name("trait").is_some())
        })
}

impl Profile for RustLang {
    fn lang(&self) -> Language {
        Language::Rust
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn definition<'t>(&self, node: Node<'t>, src: &'t [u8], parent: SymbolKind) -> Option<Def<'t>> {
        let (kind, name, child_prefix) = match node.kind() {
            "function_item" => {
                let method = matches!(parent, SymbolKind::Impl | SymbolKind::Trait);
                (
                    if method {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    },
                    text(node.child_by_field_name("name")?, src).to_owned(),
                    None,
                )
            }
            "struct_item" => (
                SymbolKind::Struct,
                text(node.child_by_field_name("name")?, src).to_owned(),
                None,
            ),
            "enum_item" => (
                SymbolKind::Enum,
                text(node.child_by_field_name("name")?, src).to_owned(),
                None,
            ),
            "trait_item" => (
                SymbolKind::Trait,
                text(node.child_by_field_name("name")?, src).to_owned(),
                None,
            ),
            "type_item" => (
                SymbolKind::TypeAlias,
                text(node.child_by_field_name("name")?, src).to_owned(),
                None,
            ),
            "const_item" | "static_item" => (
                SymbolKind::Constant,
                text(node.child_by_field_name("name")?, src).to_owned(),
                None,
            ),
            "mod_item" => {
                node.child_by_field_name("body")?;
                (
                    SymbolKind::Module,
                    text(node.child_by_field_name("name")?, src).to_owned(),
                    None,
                )
            }
            "impl_item" => {
                let ty = collapse(text(node.child_by_field_name("type")?, src));
                let name = match node.child_by_field_name("trait") {
                    Some(tr) => format!("impl {} for {ty}", collapse(text(tr, src))),
                    None => format!("impl {ty}"),
                };
                let bare = ty.split('<').next().unwrap_or(&ty).trim().to_owned();
                (SymbolKind::Impl, name, Some(bare))
            }
            _ => return None,
        };
        let attrs = attributes(node, src);
        let modifier = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "visibility_modifier")
                .map(|m| collapse(text(m, src)))
        };
        let visibility = match modifier.as_deref() {
            Some("pub") => Visibility::Public,
            Some(_) => Visibility::Internal,
            None if is_trait_impl_member(node) => Visibility::Public,
            None => Visibility::Private,
        };
        let param_count = node.child_by_field_name("parameters").map_or(0, |p| {
            let mut cursor = p.walk();
            u32::try_from(
                p.named_children(&mut cursor)
                    .filter(|c| c.kind() == "parameter")
                    .count(),
            )
            .unwrap_or(u32::MAX)
        });
        let modifiers_text = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "function_modifiers")
                .map_or(String::new(), |m| text(m, src).to_owned())
        };
        Some(Def {
            kind,
            signature: header(node, src, 300),
            visibility,
            is_async: modifiers_text.contains("async"),
            is_test: attrs.iter().any(|a| a == "test" || a.ends_with("::test")),
            param_count,
            name,
            decorators: attrs,
            node,
            span_node: node,
            child_prefix,
        })
    }

    fn is_comment(&self, kind: &str) -> bool {
        matches!(kind, "line_comment" | "block_comment")
    }

    fn is_loop(&self, kind: &str) -> bool {
        matches!(
            kind,
            "for_expression" | "while_expression" | "loop_expression"
        )
    }

    fn decision_points(&self, node: Node<'_>, src: &[u8]) -> u32 {
        match node.kind() {
            "if_expression" | "match_arm" | "for_expression" | "while_expression"
            | "loop_expression" => 1,
            "binary_expression" => u32::from(
                node.child_by_field_name("operator")
                    .is_some_and(|o| matches!(text(o, src), "&&" | "||")),
            ),
            _ => 0,
        }
    }

    fn is_nesting(&self, kind: &str) -> bool {
        matches!(
            kind,
            "if_expression"
                | "match_expression"
                | "for_expression"
                | "while_expression"
                | "loop_expression"
        )
    }

    fn call<'t>(&self, node: Node<'t>, src: &'t [u8]) -> Option<CallParts<'t>> {
        if node.kind() != "call_expression" {
            return None;
        }
        let (name, receiver) = callee(node.child_by_field_name("function")?, src)?;
        let mut args = Vec::new();
        if let Some(list) = node.child_by_field_name("arguments") {
            let mut cursor = list.walk();
            args.extend(
                list.named_children(&mut cursor)
                    .filter(|c| !matches!(c.kind(), "line_comment" | "block_comment")),
            );
        }
        Some(CallParts {
            name,
            receiver,
            args,
            kwargs: vec![],
        })
    }

    fn imports(&self, node: Node<'_>, src: &[u8]) -> Vec<ImportRef> {
        if node.kind() != "use_declaration" {
            return vec![];
        }
        let mut out = Vec::new();
        if let Some(argument) = node.child_by_field_name("argument") {
            flatten_use(argument, src, "", line_of(node), &mut out);
        }
        out
    }

    fn node_markers(&self, node: Node<'_>, src: &[u8]) -> Vec<(MarkerKind, String)> {
        match node.kind() {
            "unsafe_block" => vec![(MarkerKind::UnsafeBlock, "unsafe block".to_owned())],
            "function_item" => {
                let mut cursor = node.walk();
                let unsafe_fn = node
                    .children(&mut cursor)
                    .any(|c| c.kind() == "function_modifiers" && text(c, src).contains("unsafe"));
                if unsafe_fn {
                    vec![(MarkerKind::UnsafeBlock, "unsafe fn".to_owned())]
                } else {
                    vec![]
                }
            }
            "let_declaration" => {
                let pattern = node.child_by_field_name("pattern");
                let value = node.child_by_field_name("value");
                let mut found = Vec::new();
                if let (Some(pattern), Some(value)) = (pattern, value) {
                    if text(pattern, src) == "_"
                        && matches!(value.kind(), "call_expression" | "await_expression")
                    {
                        found.push((
                            MarkerKind::SwallowedError,
                            "a result is discarded with `let _ =`".to_owned(),
                        ));
                    }
                    if pattern.kind() == "identifier"
                        && value.kind() == "string_literal"
                        && secret_name(text(pattern, src))
                        && string_value(value, src).is_some_and(|v| plausible_secret(&v))
                    {
                        found.push((
                            MarkerKind::SecretLiteral,
                            format!("a string literal is assigned to `{}`", text(pattern, src)),
                        ));
                    }
                }
                found
            }
            "const_item" | "static_item" => {
                let (name, value) = (
                    node.child_by_field_name("name"),
                    node.child_by_field_name("value"),
                );
                match (name, value) {
                    (Some(name), Some(value))
                        if value.kind() == "string_literal"
                            && secret_name(text(name, src))
                            && string_value(value, src).is_some_and(|v| plausible_secret(&v)) =>
                    {
                        vec![(
                            MarkerKind::SecretLiteral,
                            format!("a string literal is assigned to `{}`", text(name, src)),
                        )]
                    }
                    _ => vec![],
                }
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
            && call.args.first().is_some_and(|a| {
                a.kind() == "macro_invocation"
                    && a.child_by_field_name("macro")
                        .is_some_and(|m| text(m, src) == "format")
            })
        {
            found.push((
                MarkerKind::RawSqlConcat,
                format!("`{}` receives a query built with format!", call.name),
            ));
        }
        let command_new = call.name == "new"
            && matches!(&call.receiver, Receiver::Path(p) if p.last().is_some_and(|s| s == "Command"));
        if command_new
            && call
                .args
                .first()
                .and_then(|a| string_value(*a, src))
                .is_some_and(|s| SHELLS.contains(&s.as_str()))
        {
            found.push((MarkerKind::ShellExecution, "a shell is started".to_owned()));
        }
        if call.name == "danger_accept_invalid_certs"
            && call.args.first().is_some_and(|a| text(*a, src) == "true")
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
        for (index, symbol) in ctx.symbols.iter().enumerate() {
            if symbol.kind == SymbolKind::Function
                && symbol.name == "main"
                && symbol.parent == Some(0)
            {
                hints.push(EntrypointHint {
                    symbol: Some(index),
                    scope: EntrypointScope::External,
                    kind: EntrypointKind::Main,
                    origin: EdgeOrigin::Resolved,
                    evidence: "fn main".to_owned(),
                    line: symbol.span.start_line,
                });
            }
            for attribute in &symbol.decorators {
                let Some((name, rest)) = attribute.split_once('(') else {
                    continue;
                };
                if matches!(name, "get" | "post" | "put" | "delete" | "patch") {
                    let route = rest.split('"').nth(1).unwrap_or("").to_owned();
                    hints.push(EntrypointHint {
                        symbol: Some(index),
                        scope: EntrypointScope::External,
                        kind: EntrypointKind::Http {
                            method: name.to_ascii_uppercase(),
                            route,
                        },
                        origin: EdgeOrigin::Heuristic,
                        evidence: format!("attribute #[{attribute}]"),
                        line: symbol.span.start_line,
                    });
                }
            }
        }
        hints
    }

    fn highlight(&self, node: Node<'_>, src: &[u8]) -> Option<HighlightClass> {
        match node.kind() {
            "line_comment" | "block_comment" => Some(HighlightClass::Comment),
            "string_literal" | "raw_string_literal" | "char_literal" => {
                Some(HighlightClass::String)
            }
            "integer_literal" | "float_literal" => Some(HighlightClass::Number),
            "type_identifier" | "primitive_type" => Some(HighlightClass::Type),
            "boolean_literal" => Some(HighlightClass::Keyword),
            "identifier" => {
                let parent = node.parent()?;
                (parent.kind() == "function_item"
                    && parent
                        .child_by_field_name("name")
                        .is_some_and(|n| n.id() == node.id()))
                .then_some(HighlightClass::Function)
            }
            kind if !node.is_named() && KEYWORDS.contains(&kind) && text(node, src) == kind => {
                Some(HighlightClass::Keyword)
            }
            _ => None,
        }
    }
}
