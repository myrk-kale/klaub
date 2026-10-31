//! Go (tree-sitter-go).

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

pub(crate) struct Go;

const KEYWORDS: &[&str] = &[
    "func",
    "var",
    "const",
    "type",
    "struct",
    "interface",
    "map",
    "chan",
    "package",
    "import",
    "return",
    "if",
    "else",
    "for",
    "range",
    "switch",
    "case",
    "default",
    "select",
    "go",
    "defer",
    "break",
    "continue",
    "fallthrough",
    "goto",
];
const SHELLS: &[&str] = &["sh", "bash", "zsh", "cmd", "powershell"];
const ROUTE_NAMES: &[&str] = &[
    "HandleFunc",
    "Handle",
    "GET",
    "POST",
    "PUT",
    "DELETE",
    "PATCH",
];

fn visibility(name: &str) -> Visibility {
    if name.chars().next().is_some_and(char::is_uppercase) {
        Visibility::Public
    } else {
        Visibility::Private
    }
}

fn receiver_type(node: Node<'_>, src: &[u8]) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let mut cursor = receiver.walk();
    let declaration = receiver
        .named_children(&mut cursor)
        .find(|c| c.kind() == "parameter_declaration")?;
    let ty = text(declaration.child_by_field_name("type")?, src);
    Some(
        ty.trim_start_matches('*')
            .split('[')
            .next()
            .unwrap_or(ty)
            .trim()
            .to_owned(),
    )
}

fn count_params(parameters: Option<Node<'_>>) -> u32 {
    let Some(parameters) = parameters else {
        return 0;
    };
    let mut cursor = parameters.walk();
    let mut count = 0_u32;
    for declaration in parameters.named_children(&mut cursor) {
        match declaration.kind() {
            "parameter_declaration" => {
                let mut inner = declaration.walk();
                let names = declaration
                    .children_by_field_name("name", &mut inner)
                    .count();
                count += u32::try_from(names.max(1)).unwrap_or(1);
            }
            "variadic_parameter_declaration" => count += 1,
            _ => {}
        }
    }
    count
}

fn is_dynamic_string(node: Node<'_>, src: &[u8]) -> bool {
    match node.kind() {
        "call_expression" => node
            .child_by_field_name("function")
            .is_some_and(|f| matches!(text(f, src), "fmt.Sprintf" | "fmt.Sprint" | "strings.Join")),
        "binary_expression" => {
            let literal = |n: Option<Node<'_>>| {
                n.is_some_and(|n| {
                    matches!(
                        n.kind(),
                        "interpreted_string_literal" | "raw_string_literal"
                    )
                })
            };
            node.child_by_field_name("operator")
                .is_some_and(|o| text(o, src) == "+")
                && (literal(node.child_by_field_name("left"))
                    != literal(node.child_by_field_name("right"))
                    || is_dynamic_string(node.child_by_field_name("left").unwrap_or(node), src)
                        && node
                            .child_by_field_name("left")
                            .is_some_and(|l| l.kind() == "binary_expression"))
        }
        _ => false,
    }
}

impl Profile for Go {
    fn lang(&self) -> Language {
        Language::Go
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn definition<'t>(
        &self,
        node: Node<'t>,
        src: &'t [u8],
        _parent: SymbolKind,
    ) -> Option<Def<'t>> {
        match node.kind() {
            "function_declaration" | "method_declaration" => {
                let simple = text(node.child_by_field_name("name")?, src).to_owned();
                let is_method = node.kind() == "method_declaration";
                let name = match (is_method, receiver_type(node, src)) {
                    (true, Some(ty)) => format!("{ty}.{simple}"),
                    _ => simple.clone(),
                };
                let params_text = node
                    .child_by_field_name("parameters")
                    .map_or("", |p| text(p, src));
                let is_test = ["Test", "Benchmark", "Fuzz"]
                    .iter()
                    .any(|p| simple.starts_with(p))
                    && params_text.contains("testing.");
                Some(Def {
                    kind: if is_method {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    },
                    signature: header(node, src, 300),
                    visibility: visibility(&simple),
                    decorators: vec![],
                    is_async: false,
                    is_test,
                    param_count: count_params(node.child_by_field_name("parameters")),
                    name,
                    node,
                    span_node: node,
                    child_prefix: None,
                })
            }
            "type_spec" => {
                let name = text(node.child_by_field_name("name")?, src).to_owned();
                let kind = match node.child_by_field_name("type").map(|t| t.kind()) {
                    Some("struct_type") => SymbolKind::Struct,
                    Some("interface_type") => SymbolKind::Interface,
                    _ => SymbolKind::TypeAlias,
                };
                let declaration = node.parent().filter(|p| p.kind() == "type_declaration");
                let single = declaration.is_some_and(|d| {
                    let mut cursor = d.walk();
                    d.children(&mut cursor)
                        .filter(|c| c.kind() == "type_spec")
                        .count()
                        == 1
                });
                let head = match node.child_by_field_name("type") {
                    Some(t) if matches!(t.kind(), "struct_type" | "interface_type") => {
                        format!("type {name} {}", t.kind().trim_end_matches("_type"))
                    }
                    Some(t) => collapse(&format!("type {name} {}", text(t, src))),
                    None => format!("type {name}"),
                };
                Some(Def {
                    kind,
                    signature: head,
                    visibility: visibility(&name),
                    decorators: vec![],
                    is_async: false,
                    is_test: false,
                    param_count: 0,
                    name,
                    node,
                    span_node: if single {
                        declaration.unwrap_or(node)
                    } else {
                        node
                    },
                    child_prefix: None,
                })
            }
            _ => None,
        }
    }

    fn is_comment(&self, kind: &str) -> bool {
        kind == "comment"
    }

    fn is_loop(&self, kind: &str) -> bool {
        kind == "for_statement"
    }

    fn decision_points(&self, node: Node<'_>, src: &[u8]) -> u32 {
        match node.kind() {
            "if_statement" | "for_statement" | "expression_case" | "type_case"
            | "communication_case" => 1,
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
            "if_statement"
                | "for_statement"
                | "expression_switch_statement"
                | "type_switch_statement"
                | "select_statement"
        )
    }

    fn call<'t>(&self, node: Node<'t>, src: &'t [u8]) -> Option<CallParts<'t>> {
        if node.kind() != "call_expression" {
            return None;
        }
        let function = node.child_by_field_name("function")?;
        let (name, receiver) = match function.kind() {
            "identifier" => (text(function, src).to_owned(), Receiver::None),
            "selector_expression" => {
                let name = text(function.child_by_field_name("field")?, src).to_owned();
                let operand = function.child_by_field_name("operand")?;
                let receiver = match operand.kind() {
                    "identifier" => Receiver::Path(vec![text(operand, src).to_owned()]),
                    "selector_expression" => {
                        let flat: Vec<String> = text(operand, src)
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
                (name, receiver)
            }
            _ => return None,
        };
        let mut args = Vec::new();
        if let Some(list) = node.child_by_field_name("arguments") {
            let mut cursor = list.walk();
            args.extend(
                list.named_children(&mut cursor)
                    .filter(|c| c.kind() != "comment"),
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
        if node.kind() != "import_spec" {
            return vec![];
        }
        let Some(module) = node
            .child_by_field_name("path")
            .and_then(|p| string_value(p, src))
        else {
            return vec![];
        };
        let name = node
            .child_by_field_name("name")
            .map(|n| text(n, src).to_owned());
        let wildcard = name.as_deref() == Some(".");
        let alias = name.filter(|n| n != "." && n != "_");
        vec![ImportRef {
            module,
            relative_level: 0,
            names: Vec::<ImportedName>::new(),
            alias,
            wildcard,
            reexport: false,
            dynamic: false,
            line: line_of(node),
        }]
    }

    fn node_markers(&self, node: Node<'_>, src: &[u8]) -> Vec<(MarkerKind, String)> {
        match node.kind() {
            "if_statement" => {
                let condition = node
                    .child_by_field_name("condition")
                    .map_or(String::new(), |c| collapse(text(c, src)));
                let empty = node.child_by_field_name("consequence").is_some_and(|b| {
                    let mut cursor = b.walk();
                    b.named_children(&mut cursor).all(|c| c.kind() == "comment")
                });
                if empty && condition.contains("err != nil") {
                    vec![(
                        MarkerKind::SwallowedError,
                        "an `if err != nil` block does nothing".to_owned(),
                    )]
                } else {
                    vec![]
                }
            }
            "short_var_declaration"
            | "assignment_statement"
            | "var_spec"
            | "const_spec"
            | "keyed_element" => {
                let (name, value) = match node.kind() {
                    "short_var_declaration" | "assignment_statement" => (
                        node.child_by_field_name("left"),
                        node.child_by_field_name("right"),
                    ),
                    "keyed_element" => (node.named_child(0), node.named_child(1)),
                    _ => (
                        node.child_by_field_name("name"),
                        node.child_by_field_name("value"),
                    ),
                };
                let (Some(name), Some(value)) = (name, value) else {
                    return vec![];
                };
                let name = text(name, src).trim().to_owned();
                let literal = value.named_child(0).unwrap_or(value);
                let mut found = Vec::new();
                if node.kind() == "keyed_element"
                    && name.ends_with("InsecureSkipVerify")
                    && text(literal, src) == "true"
                {
                    found.push((
                        MarkerKind::VerificationDisabled,
                        "certificate verification is turned off".to_owned(),
                    ));
                }
                if matches!(
                    literal.kind(),
                    "interpreted_string_literal" | "raw_string_literal"
                ) && secret_name(&name)
                    && string_value(literal, src).is_some_and(|v| plausible_secret(&v))
                {
                    found.push((
                        MarkerKind::SecretLiteral,
                        format!("a string literal is assigned to `{name}`"),
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
        let exec_command = call.name == "Command"
            && matches!(&call.receiver, Receiver::Path(p) if p.last().is_some_and(|s| s == "exec"));
        if exec_command
            && call
                .args
                .first()
                .and_then(|a| string_value(*a, src))
                .is_some_and(|s| SHELLS.contains(&s.as_str()))
        {
            found.push((MarkerKind::ShellExecution, "a shell is started".to_owned()));
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
        let package_main = {
            let mut cursor = ctx.root.walk();
            ctx.root.children(&mut cursor).any(|c| {
                c.kind() == "package_clause"
                    && text(c, ctx.src).split_whitespace().nth(1) == Some("main")
            })
        };
        if package_main
            && let Some(index) = ctx
                .symbols
                .iter()
                .position(|s| s.kind == SymbolKind::Function && s.name == "main")
        {
            hints.push(EntrypointHint {
                symbol: Some(index),
                scope: EntrypointScope::External,
                kind: EntrypointKind::Main,
                origin: EdgeOrigin::Resolved,
                evidence: "func main in package main".to_owned(),
                line: ctx.symbols[index].span.start_line,
            });
        }
        let mut stack = vec![ctx.root];
        while let Some(node) = stack.pop() {
            if node.kind() == "call_expression"
                && let Some(function) = node
                    .child_by_field_name("function")
                    .filter(|f| f.kind() == "selector_expression")
                && let Some(field) = function
                    .child_by_field_name("field")
                    .map(|f| text(f, ctx.src))
                && ROUTE_NAMES.contains(&field)
                && let Some(arguments) = node.child_by_field_name("arguments")
                && let Some(route) = arguments
                    .named_child(0)
                    .and_then(|a| string_value(a, ctx.src))
                    .filter(|r| r.starts_with('/'))
                && let Some(handler) = arguments
                    .named_child(1)
                    .filter(|h| h.kind() == "identifier")
                && let Some(index) = ctx
                    .symbols
                    .iter()
                    .position(|s| s.name == text(handler, ctx.src))
            {
                let method = if matches!(field, "Handle" | "HandleFunc") {
                    "ANY".to_owned()
                } else {
                    field.to_owned()
                };
                hints.push(EntrypointHint {
                    symbol: Some(index),
                    scope: EntrypointScope::External,
                    kind: EntrypointKind::Http { method, route },
                    origin: EdgeOrigin::Heuristic,
                    evidence: format!("{}(...)", text(function, ctx.src)),
                    line: line_of(node),
                });
            }
            let mut cursor = node.walk();
            stack.extend(node.children(&mut cursor));
        }
        hints
    }

    fn highlight(&self, node: Node<'_>, src: &[u8]) -> Option<HighlightClass> {
        match node.kind() {
            "comment" => Some(HighlightClass::Comment),
            "interpreted_string_literal" | "raw_string_literal" | "rune_literal" => {
                Some(HighlightClass::String)
            }
            "int_literal" | "float_literal" | "imaginary_literal" => Some(HighlightClass::Number),
            "type_identifier" => Some(HighlightClass::Type),
            "true" | "false" | "nil" => Some(HighlightClass::Keyword),
            "identifier" | "field_identifier" => {
                let parent = node.parent()?;
                (matches!(parent.kind(), "function_declaration" | "method_declaration")
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
