//! Java (tree-sitter-java).
//!
//! Java has no free functions, so every method and constructor is a [`SymbolKind::Method`] under
//! its type, and nested types qualify their members (`Outer.Inner.run`). Annotations are kept as
//! decorators without the `@`, which is what the framework entrypoint detection reads. Nothing
//! here follows reflection, dependency injection or code generation: a call that goes through them
//! stays unresolved, and the impact view says so.

use klaub_core::graph::{EdgeOrigin, EntrypointKind, EntrypointScope};
use klaub_core::language::Language;
use klaub_core::symbol::{
    EntrypointHint, HighlightClass, ImportRef, ImportedName, MarkerKind, Receiver, SymbolKind,
    Visibility,
};
use tree_sitter::Node;

use crate::engine::{CallParts, Def, EntryCtx, Profile, collapse, line_of, string_value, text};
use crate::markers::{exception_name, plausible_secret, receiver_last, secret_name, sql_sink};

pub(crate) struct Java;

/// Keywords that appear as tokens of their own. The primitive types are left out because the
/// grammar gives them a node of their own, which is coloured as a type.
const KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "do",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "for",
    "if",
    "implements",
    "import",
    "instanceof",
    "interface",
    "native",
    "new",
    "package",
    "permits",
    "private",
    "protected",
    "public",
    "record",
    "return",
    "sealed",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "var",
    "volatile",
    "while",
    "yield",
];
const SHELLS: &[&str] = &["sh", "bash", "zsh", "cmd", "cmd.exe", "powershell"];
const SPRING_MAPPINGS: &[(&str, &str)] = &[
    ("GetMapping", "GET"),
    ("PostMapping", "POST"),
    ("PutMapping", "PUT"),
    ("DeleteMapping", "DELETE"),
    ("PatchMapping", "PATCH"),
    ("RequestMapping", "ANY"),
];
const JAXRS_METHODS: &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"];
const TEST_ANNOTATIONS: &[&str] = &[
    "Test",
    "ParameterizedTest",
    "RepeatedTest",
    "TestFactory",
    "TestTemplate",
];
/// Receivers whose methods read or write something outside the process.
const IO_RECEIVERS: &[&str] = &[
    "db",
    "database",
    "dao",
    "statement",
    "stmt",
    "preparedstatement",
    "connection",
    "conn",
    "jdbctemplate",
    "resttemplate",
    "webclient",
    "httpclient",
    "client",
    "entitymanager",
    "em",
    "session",
    "repository",
    "repo",
    "files",
];
const IO_METHODS: &[&str] = &[
    "executequery",
    "executeupdate",
    "execute",
    "query",
    "queryforobject",
    "queryforlist",
    "update",
    "send",
    "exchange",
    "getforobject",
    "postforobject",
    "retrieve",
    "persist",
    "merge",
    "find",
    "save",
    "saveall",
    "findbyid",
    "findall",
    "readallbytes",
    "readalllines",
    "readstring",
    "write",
    "writestring",
    "newbufferedreader",
    "newbufferedwriter",
];

/// The annotations of a declaration and its modifier keywords. Annotations are written without
/// the `@`, with their arguments on one line.
fn modifiers(node: Node<'_>, src: &[u8]) -> (Vec<String>, Vec<String>) {
    let mut annotations = Vec::new();
    let mut keywords = Vec::new();
    let mut collect = |child: Node<'_>| {
        if matches!(child.kind(), "annotation" | "marker_annotation") {
            annotations.push(collapse(text(child, src).trim_start_matches('@')));
        } else if !child.is_named() {
            keywords.push(text(child, src).to_owned());
        }
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut inner = child.walk();
            for modifier in child.children(&mut inner) {
                collect(modifier);
            }
        } else if matches!(child.kind(), "annotation" | "marker_annotation") {
            collect(child);
        }
    }
    (annotations, keywords)
}

/// The simple name of an annotation: `GetMapping("/x")` is `GetMapping`, `a.b.Test` is `Test`.
fn annotation_name(annotation: &str) -> &str {
    let name = annotation.split('(').next().unwrap_or(annotation).trim();
    name.rsplit('.').next().unwrap_or(name)
}

/// The declaration without annotations and without the body.
fn signature(node: Node<'_>, src: &[u8], keywords: &[String]) -> String {
    let mut cursor = node.walk();
    let start = node
        .children(&mut cursor)
        .find(|c| !matches!(c.kind(), "modifiers" | "annotation" | "marker_annotation"))
        .map_or(node.start_byte(), |c| c.start_byte());
    let end = node
        .child_by_field_name("body")
        .map_or(node.end_byte(), |b| b.start_byte());
    let rest =
        collapse(std::str::from_utf8(src.get(start..end.max(start)).unwrap_or(&[])).unwrap_or(""));
    let rest = rest.trim_end_matches([';', '{', ' ']);
    let mut out = keywords.join(" ");
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(rest);
    if out.chars().count() > 300 {
        out.chars().take(300).collect()
    } else {
        out
    }
}

fn visibility(keywords: &[String], parent: SymbolKind) -> Visibility {
    if keywords.iter().any(|k| k == "public") {
        Visibility::Public
    } else if keywords.iter().any(|k| k == "protected") {
        Visibility::Protected
    } else if keywords.iter().any(|k| k == "private") {
        Visibility::Private
    } else if parent == SymbolKind::Interface {
        // Interface members are public without saying so.
        Visibility::Public
    } else {
        Visibility::Internal
    }
}

fn count_params(parameters: Option<Node<'_>>) -> u32 {
    let Some(parameters) = parameters else {
        return 0;
    };
    let mut cursor = parameters.walk();
    let count = parameters
        .named_children(&mut cursor)
        .filter(|c| matches!(c.kind(), "formal_parameter" | "spread_parameter"))
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The type a `new` expression creates, without generics or the package: `new a.B<C>()` is `B`.
fn created_type(node: Node<'_>, src: &[u8]) -> Option<String> {
    let ty = node.child_by_field_name("type")?;
    let raw = text(ty, src);
    let bare = raw.split('<').next().unwrap_or(raw).trim();
    let simple = bare.rsplit('.').next().unwrap_or(bare);
    (!simple.is_empty()).then(|| simple.to_owned())
}

fn is_string(node: Node<'_>) -> bool {
    node.kind() == "string_literal"
}

/// Whether a string is built from pieces at run time: `"a" + b`, `String.format(...)`.
fn is_dynamic_string(node: Node<'_>, src: &[u8]) -> bool {
    match node.kind() {
        "binary_expression" => {
            node.child_by_field_name("operator")
                .is_some_and(|o| text(o, src) == "+")
                && (node.child_by_field_name("left").is_some_and(is_string)
                    || node.child_by_field_name("right").is_some_and(is_string))
                && !(node.child_by_field_name("left").is_some_and(is_string)
                    && node.child_by_field_name("right").is_some_and(is_string))
        }
        "method_invocation" => {
            let name = node
                .child_by_field_name("name")
                .map_or("", |n| text(n, src));
            matches!(name, "format" | "formatted" | "concat")
        }
        _ => false,
    }
}

impl Profile for Java {
    fn lang(&self) -> Language {
        Language::Java
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn definition<'t>(&self, node: Node<'t>, src: &'t [u8], parent: SymbolKind) -> Option<Def<'t>> {
        let kind = match node.kind() {
            "class_declaration" => SymbolKind::Class,
            "interface_declaration" | "annotation_type_declaration" => SymbolKind::Interface,
            "enum_declaration" => SymbolKind::Enum,
            "record_declaration" => SymbolKind::Struct,
            "method_declaration" | "constructor_declaration" => SymbolKind::Method,
            _ => return None,
        };
        let name = text(node.child_by_field_name("name")?, src).to_owned();
        let (annotations, keywords) = modifiers(node, src);
        let is_test = kind == SymbolKind::Method
            && annotations
                .iter()
                .any(|a| TEST_ANNOTATIONS.contains(&annotation_name(a)));
        let param_count = if kind == SymbolKind::Method {
            count_params(node.child_by_field_name("parameters"))
        } else {
            0
        };
        Some(Def {
            kind,
            signature: signature(node, src, &keywords),
            visibility: visibility(&keywords, parent),
            decorators: annotations,
            is_async: false,
            is_test,
            param_count,
            name,
            node,
            span_node: node,
            child_prefix: None,
        })
    }

    fn is_comment(&self, kind: &str) -> bool {
        matches!(kind, "line_comment" | "block_comment")
    }

    fn is_loop(&self, kind: &str) -> bool {
        matches!(
            kind,
            "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
        )
    }

    fn decision_points(&self, node: Node<'_>, src: &[u8]) -> u32 {
        match node.kind() {
            "if_statement"
            | "for_statement"
            | "enhanced_for_statement"
            | "while_statement"
            | "do_statement"
            | "catch_clause"
            | "ternary_expression"
            | "switch_block_statement_group"
            | "switch_rule" => 1,
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
                | "enhanced_for_statement"
                | "while_statement"
                | "do_statement"
                | "switch_expression"
                | "try_statement"
        )
    }

    fn call<'t>(&self, node: Node<'t>, src: &'t [u8]) -> Option<CallParts<'t>> {
        let (name, receiver) = match node.kind() {
            "method_invocation" => {
                let name = text(node.child_by_field_name("name")?, src).to_owned();
                let receiver = match node.child_by_field_name("object") {
                    None => Receiver::None,
                    Some(object) => match object.kind() {
                        "this" | "super" => Receiver::SelfRef,
                        "identifier" | "field_access" => {
                            let flat: Vec<String> = text(object, src)
                                .split('.')
                                .map(|s| s.trim().to_owned())
                                .collect();
                            if flat.iter().all(|s| {
                                !s.is_empty()
                                    && s.chars()
                                        .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                            }) {
                                Receiver::Path(flat)
                            } else {
                                Receiver::Complex
                            }
                        }
                        _ => Receiver::Complex,
                    },
                };
                (name, receiver)
            }
            // `new Foo(...)` calls the constructor, which is a method called `Foo` of the class.
            "object_creation_expression" => (created_type(node, src)?, Receiver::None),
            _ => return None,
        };
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
        if node.kind() != "import_declaration" {
            return vec![];
        }
        let mut cursor = node.walk();
        let mut is_static = false;
        let mut wildcard = false;
        let mut path = None;
        for child in node.children(&mut cursor) {
            match child.kind() {
                "static" => is_static = true,
                "asterisk" => wildcard = true,
                "scoped_identifier" | "identifier" => path = Some(text(child, src).to_owned()),
                _ => {}
            }
        }
        let Some(path) = path else {
            return vec![];
        };
        // `import static a.B.run;` names a member of `a.B`; every other form names the class or,
        // with `*`, the package.
        let (module, names) = if is_static && !wildcard {
            match path.rsplit_once('.') {
                Some((class, member)) => (
                    class.to_owned(),
                    vec![ImportedName {
                        name: member.to_owned(),
                        alias: None,
                    }],
                ),
                None => (path, vec![]),
            }
        } else {
            (path, vec![])
        };
        vec![ImportRef {
            module,
            relative_level: 0,
            names,
            alias: None,
            wildcard,
            reexport: false,
            dynamic: false,
            line: line_of(node),
        }]
    }

    fn raised(&self, node: Node<'_>, src: &[u8]) -> Option<String> {
        if node.kind() != "throw_statement" {
            return None;
        }
        let mut cursor = node.walk();
        let expression = node.named_children(&mut cursor).next()?;
        // Only `throw new X(...)` names an exception; `throw e` rethrows something else.
        if expression.kind() != "object_creation_expression" {
            return None;
        }
        let ty = expression.child_by_field_name("type")?;
        let raw = text(ty, src);
        exception_name(raw.split('<').next().unwrap_or(raw))
    }

    fn node_markers(&self, node: Node<'_>, src: &[u8]) -> Vec<(MarkerKind, String)> {
        match node.kind() {
            "catch_clause" => {
                let empty = node.child_by_field_name("body").is_some_and(|body| {
                    let mut cursor = body.walk();
                    body.named_children(&mut cursor)
                        .all(|c| matches!(c.kind(), "line_comment" | "block_comment"))
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
            "variable_declarator" => {
                let (Some(name), Some(value)) = (
                    node.child_by_field_name("name"),
                    node.child_by_field_name("value"),
                ) else {
                    return vec![];
                };
                let name = text(name, src);
                if is_string(value)
                    && secret_name(name)
                    && string_value(value, src).is_some_and(|v| plausible_secret(&v))
                {
                    vec![(
                        MarkerKind::SecretLiteral,
                        format!("a string literal is assigned to `{name}`"),
                    )]
                } else {
                    vec![]
                }
            }
            "object_creation_expression" => {
                let is_process_builder =
                    created_type(node, src).as_deref() == Some("ProcessBuilder");
                let shell = node
                    .child_by_field_name("arguments")
                    .and_then(|list| list.named_child(0))
                    .and_then(|first| string_value(first, src))
                    .is_some_and(|s| SHELLS.contains(&s.as_str()));
                if is_process_builder && shell {
                    vec![(MarkerKind::ShellExecution, "a shell is started".to_owned())]
                } else {
                    vec![]
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
        // `Runtime.getRuntime().exec("sh -c ...")`: the receiver is a call result, so only the
        // method name and a first argument that starts a shell can be told apart.
        if call.name == "exec"
            && call.args.first().is_some_and(|a| {
                string_value(*a, src).is_some_and(|s| {
                    let program = s.split_whitespace().next().unwrap_or("");
                    SHELLS.contains(&program)
                })
            })
        {
            found.push((MarkerKind::ShellExecution, "a shell is started".to_owned()));
        }
        if in_loop && is_io(&call.receiver, &call.name) {
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
            if symbol.kind != SymbolKind::Method {
                continue;
            }
            if symbol.name == "main"
                && symbol.signature.contains("static")
                && symbol.signature.contains("String")
            {
                hints.push(EntrypointHint {
                    symbol: Some(index),
                    scope: EntrypointScope::External,
                    kind: EntrypointKind::Main,
                    origin: EdgeOrigin::Resolved,
                    evidence: "public static void main(String[])".to_owned(),
                    line: symbol.span.start_line,
                });
            }
            let prefix = symbol
                .parent
                .map(|p| class_route(&ctx.symbols[p].decorators))
                .unwrap_or_default();

            for decorator in &symbol.decorators {
                let name = annotation_name(decorator);
                let Some(&(_, spring_method)) = SPRING_MAPPINGS
                    .iter()
                    .find(|(annotation, _)| *annotation == name)
                else {
                    continue;
                };
                let method = if spring_method == "ANY" {
                    request_method(decorator).unwrap_or_else(|| "ANY".to_owned())
                } else {
                    spring_method.to_owned()
                };
                hints.push(EntrypointHint {
                    symbol: Some(index),
                    scope: EntrypointScope::External,
                    kind: EntrypointKind::Http {
                        method,
                        route: join_route(&prefix, &first_string(decorator)),
                    },
                    origin: EdgeOrigin::Heuristic,
                    evidence: format!("annotation @{}", collapse(decorator)),
                    line: symbol.span.start_line,
                });
            }

            // JAX-RS: `@GET` (or another verb) with an optional `@Path` on the method.
            if let Some(verb) = symbol
                .decorators
                .iter()
                .map(|d| annotation_name(d))
                .find(|d| JAXRS_METHODS.contains(d))
            {
                let own = symbol
                    .decorators
                    .iter()
                    .find(|d| annotation_name(d) == "Path")
                    .map(|d| first_string(d))
                    .unwrap_or_default();
                hints.push(EntrypointHint {
                    symbol: Some(index),
                    scope: EntrypointScope::External,
                    kind: EntrypointKind::Http {
                        method: verb.to_owned(),
                        route: join_route(&prefix, &own),
                    },
                    origin: EdgeOrigin::Heuristic,
                    evidence: format!("annotation @{verb}"),
                    line: symbol.span.start_line,
                });
            }
        }
        let _ = ctx.root;
        hints
    }

    fn highlight(&self, node: Node<'_>, src: &[u8]) -> Option<HighlightClass> {
        match node.kind() {
            "line_comment" | "block_comment" => Some(HighlightClass::Comment),
            "string_literal" | "character_literal" => Some(HighlightClass::String),
            "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal" => Some(HighlightClass::Number),
            "type_identifier"
            | "integral_type"
            | "floating_point_type"
            | "boolean_type"
            | "void_type" => Some(HighlightClass::Type),
            "true" | "false" | "null_literal" => Some(HighlightClass::Keyword),
            "identifier" => {
                let parent = node.parent()?;
                (matches!(
                    parent.kind(),
                    "method_declaration" | "constructor_declaration" | "method_invocation"
                ) && parent
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

/// Whether a call looks like network, file or database I/O.
fn is_io(receiver: &Receiver, name: &str) -> bool {
    let Some(last) = receiver_last(receiver).map(str::to_ascii_lowercase) else {
        return false;
    };
    IO_RECEIVERS.contains(&last.as_str())
        && IO_METHODS.contains(&name.to_ascii_lowercase().as_str())
}

/// The first string literal inside an annotation's arguments: `GetMapping("/users")`,
/// `RequestMapping(value = "/users")` and `Path("/users")` give `/users`.
fn first_string(annotation: &str) -> String {
    annotation.split('"').nth(1).unwrap_or_default().to_owned()
}

/// The verb a `@RequestMapping(method = RequestMethod.POST)` names.
fn request_method(annotation: &str) -> Option<String> {
    let after = annotation.split("RequestMethod.").nth(1)?;
    let verb: String = after
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect();
    (!verb.is_empty()).then(|| verb.to_ascii_uppercase())
}

/// The route a class puts in front of its methods' routes.
fn class_route(decorators: &[String]) -> String {
    decorators
        .iter()
        .find(|d| matches!(annotation_name(d), "RequestMapping" | "Path"))
        .map(|d| first_string(d))
        .unwrap_or_default()
}

fn join_route(prefix: &str, route: &str) -> String {
    let joined = format!("/{}/{}", prefix.trim_matches('/'), route.trim_matches('/'));
    let collapsed = joined.replace("//", "/");
    if collapsed.len() > 1 {
        collapsed.trim_end_matches('/').to_owned()
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::join_route;

    #[test]
    fn routes_are_joined_with_single_slashes() {
        assert_eq!(join_route("", ""), "/");
        assert_eq!(join_route("/api", "/users"), "/api/users");
        assert_eq!(join_route("api/", "users/"), "/api/users");
        assert_eq!(join_route("", "/x"), "/x");
        assert_eq!(join_route("/api", ""), "/api");
    }
}
