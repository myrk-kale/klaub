//! Cuts a normalised declaration into its parameters and its return type.
//!
//! A signature arrives as one string whose shape depends on the language (`def f(a) -> b`,
//! `fn f<T>(a: T) -> b where ...`, `function f(a): b`, `func (r T) f(a int) (int, error)`). The
//! cut is done by reading the string the way the language writes it: find the parameter list, split
//! it on the commas that are not nested in brackets, strings or generics, and read what follows.
//! When the shape is not one of those, [`split`] says so by returning `None`; it never guesses.

use klaub_core::language::Language;
use klaub_core::symbol::SymbolKind;

/// A declaration split into the parts a contract is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParts {
    /// The parameters as written, in order, without the surrounding parentheses.
    pub parameters: Vec<String>,
    /// The declared return type, when the declaration states one.
    pub returns: Option<String>,
}

/// Splits `signature`, or returns `None` when `kind` is not callable or the text has no parameter
/// list this reader understands.
pub(crate) fn split(
    language: Language,
    kind: SymbolKind,
    signature: &str,
) -> Option<SignatureParts> {
    if !matches!(kind, SymbolKind::Function | SymbolKind::Method) {
        return None;
    }
    let text = signature.trim();
    let mut open = first_open_paren(text, 0)?;
    if language == Language::Go && text.starts_with("func (") {
        // `func (s *Server) Handle(a int)`: the first group is the receiver, not the parameters.
        let receiver_end = matching_paren(text, open)?;
        open = first_open_paren(text, receiver_end + 1)?;
    }
    let close = matching_paren(text, open)?;
    let inside = text.get(open + 1..close)?;
    let parameters = split_top_level(inside);
    let rest = text.get(close + 1..)?.trim();
    Some(SignatureParts {
        parameters,
        returns: return_type(language, rest),
    })
}

/// The index of the first `(` at or after `from` that is not inside a generic list.
fn first_open_paren(text: &str, from: usize) -> Option<usize> {
    let mut angle = 0_i32;
    let mut previous = ' ';
    for (index, c) in text.char_indices().skip_while(|(i, _)| *i < from) {
        match c {
            '<' => angle += 1,
            '>' if previous != '-' && previous != '=' => angle = (angle - 1).max(0),
            '(' if angle == 0 => return Some(index),
            _ => {}
        }
        previous = c;
    }
    None
}

/// The index of the `)` that closes the `(` at `open`, skipping strings and nested brackets.
fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0_i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in text.char_indices().skip_while(|(i, _)| *i < open) {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' | '`' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return (c == ')').then_some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits on the commas that are not inside brackets, generics or strings.
fn split_top_level(inside: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    let mut angle = 0_i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut previous = ' ';
    for c in inside.chars() {
        if let Some(q) = quote {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            previous = c;
            continue;
        }
        match c {
            '"' | '\'' | '`' => {
                quote = Some(c);
                current.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            '<' => {
                angle += 1;
                current.push(c);
            }
            '>' if previous != '-' && previous != '=' => {
                angle = (angle - 1).max(0);
                current.push(c);
            }
            ',' if depth == 0 && angle == 0 => {
                push_part(&mut parts, &mut current);
            }
            _ => current.push(c),
        }
        previous = c;
    }
    push_part(&mut parts, &mut current);
    parts
}

fn push_part(parts: &mut Vec<String>, current: &mut String) {
    let part = current.trim();
    if !part.is_empty() {
        parts.push(part.to_owned());
    }
    current.clear();
}

/// What follows the closing parenthesis, read the way `language` writes a return type.
fn return_type(language: Language, rest: &str) -> Option<String> {
    let raw = match language {
        Language::Python | Language::Rust => rest.strip_prefix("->")?,
        Language::JavaScript | Language::TypeScript | Language::Tsx => rest.strip_prefix(':')?,
        Language::Go => rest,
    };
    let mut text = raw;
    for stop in [" where ", "{", "=>"] {
        if let Some(at) = text.find(stop) {
            text = &text[..at];
        }
    }
    let text = text.trim().trim_end_matches(':').trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(language: Language, kind: SymbolKind, signature: &str) -> SignatureParts {
        split(language, kind, signature).expect("the signature should split")
    }

    #[test]
    fn python_parameters_and_return() {
        let p = parts(
            Language::Python,
            SymbolKind::Method,
            "async def fetch(self, key: str, opts: dict[str, int] = {}) -> Optional[str]:",
        );
        assert_eq!(
            p.parameters,
            vec!["self", "key: str", "opts: dict[str, int] = {}"]
        );
        assert_eq!(p.returns.as_deref(), Some("Optional[str]"));
    }

    #[test]
    fn a_declaration_without_a_return_type_has_none() {
        let p = parts(Language::Python, SymbolKind::Function, "def f(a, b)");
        assert_eq!(p.parameters, vec!["a", "b"]);
        assert_eq!(p.returns, None);
    }

    #[test]
    fn rust_generics_do_not_hide_the_parameter_list() {
        let p = parts(
            Language::Rust,
            SymbolKind::Function,
            "pub fn run<F: Fn(u8) -> u8, T>(f: F, items: Vec<(u8, T)>) -> Result<T, Error> where T: Clone",
        );
        assert_eq!(p.parameters, vec!["f: F", "items: Vec<(u8, T)>"]);
        assert_eq!(p.returns.as_deref(), Some("Result<T, Error>"));
    }

    #[test]
    fn typescript_return_type_follows_a_colon() {
        let p = parts(
            Language::TypeScript,
            SymbolKind::Function,
            "export async function load(id: string, opts?: { deep: boolean }): Promise<User>",
        );
        assert_eq!(p.parameters, vec!["id: string", "opts?: { deep: boolean }"]);
        assert_eq!(p.returns.as_deref(), Some("Promise<User>"));
    }

    #[test]
    fn go_skips_the_receiver_and_keeps_a_tuple_return() {
        let p = parts(
            Language::Go,
            SymbolKind::Method,
            "func (s *Server) Handle(ctx context.Context, n int) (int, error)",
        );
        assert_eq!(p.parameters, vec!["ctx context.Context", "n int"]);
        assert_eq!(p.returns.as_deref(), Some("(int, error)"));
    }

    #[test]
    fn a_string_default_with_a_comma_stays_in_one_parameter() {
        let p = parts(
            Language::Python,
            SymbolKind::Function,
            "def f(sep=\", \", n=1) -> str",
        );
        assert_eq!(p.parameters, vec!["sep=\", \"", "n=1"]);
    }

    #[test]
    fn no_parameters_is_an_empty_list_not_unknown() {
        let p = parts(Language::Rust, SymbolKind::Function, "fn main()");
        assert!(p.parameters.is_empty());
    }

    #[test]
    fn what_cannot_be_read_is_none() {
        assert_eq!(
            split(Language::Python, SymbolKind::Class, "class A(B)"),
            None
        );
        assert_eq!(
            split(Language::Python, SymbolKind::Function, "lambda x: x"),
            None
        );
        assert_eq!(
            split(Language::Python, SymbolKind::Function, "def f(a"),
            None
        );
    }
}
