//! The documentation comment of a symbol, read from the lines around it.
//!
//! No grammar is involved: a doc comment is either a run of comment lines right above the symbol
//! (`///`, `//`, `#`, a `/** */` block) or, in Python, a string that opens the body. Reading the text
//! keeps this working for every language the reader shows, at the cost of being plain about what it
//! cannot tell: a comment that merely sits above a symbol is returned as its documentation.

/// The comment that documents the symbol starting at `start_line` (1-based) of `text`, with the
/// comment markers removed, or `None` when there is none.
pub(crate) fn extract(text: &str, start_line: u32) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = usize::try_from(start_line).ok()?.checked_sub(1)?;
    if start >= lines.len() {
        return None;
    }
    above(&lines, start).or_else(|| docstring(&lines, start))
}

/// The run of comment lines directly above `start`, skipping attributes and decorators.
fn above(lines: &[&str], start: usize) -> Option<String> {
    let mut found: Vec<String> = Vec::new();
    for line in lines[..start].iter().rev() {
        let line = line.trim();
        if found.is_empty() && is_annotation(line) {
            continue;
        }
        match strip_marker(line) {
            Some(text) => found.push(text),
            None => break,
        }
    }
    found.reverse();
    tidy(found)
}

/// `#[derive(..)]`, `@decorator`: lines that sit between a doc comment and its symbol.
fn is_annotation(line: &str) -> bool {
    line.starts_with("#[") || line.starts_with("#!") || line.starts_with('@')
}

/// The text of a comment line, or `None` when the line is not a comment.
fn strip_marker(line: &str) -> Option<String> {
    let rest = if let Some(r) = line
        .strip_prefix("///")
        .or_else(|| line.strip_prefix("//!"))
    {
        r
    } else if let Some(r) = line.strip_prefix("//") {
        r
    } else if let Some(r) = line.strip_prefix("/**").or_else(|| line.strip_prefix("/*")) {
        r.trim_end_matches("*/")
    } else if line == "*/" {
        ""
    } else if let Some(r) = line.strip_prefix('*') {
        r.trim_end_matches("*/")
    } else {
        line.strip_prefix('#')?
    };
    Some(rest.strip_prefix(' ').unwrap_or(rest).trim_end().to_owned())
}

/// A Python-style string that opens the body of the definition starting at `start`.
fn docstring(lines: &[&str], start: usize) -> Option<String> {
    let first = lines[start].trim_start();
    if !["def ", "async def ", "class "]
        .iter()
        .any(|keyword| first.starts_with(keyword))
    {
        return None;
    }
    // The signature may take several lines; the body starts after the one ending in `:`.
    let signature_end =
        (start..lines.len().min(start + 16)).find(|&i| lines[i].trim_end().ends_with(':'))?;
    let body = lines[signature_end + 1..]
        .iter()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty())?;
    let (offset, opening) = body;
    let opening = opening.trim_start();
    let opening = opening.trim_start_matches(['r', 'R', 'b', 'B', 'u', 'U']);
    let quote = ["\"\"\"", "'''"]
        .into_iter()
        .find(|q| opening.starts_with(q))?;
    let after_open = &opening[quote.len()..];
    if let Some(end) = after_open.find(quote) {
        return tidy(vec![after_open[..end].trim().to_owned()]);
    }
    let mut collected = vec![after_open.trim().to_owned()];
    for line in &lines[signature_end + 1 + offset + 1..] {
        if let Some(end) = line.find(quote) {
            collected.push(line[..end].trim().to_owned());
            return tidy(collected);
        }
        collected.push(line.trim().to_owned());
    }
    // Never closed: it is not a docstring this reader can bound.
    None
}

/// Drops empty lines at both ends; `None` when nothing is left.
fn tidy(mut lines: Vec<String>) -> Option<String> {
    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_doc_lines_above_an_attribute_belong_to_the_symbol() {
        let text = "fn other() {}\n\n/// Adds one.\n///\n/// Second paragraph.\n#[inline]\nfn add(x: u32) -> u32 {\n    x + 1\n}\n";
        assert_eq!(
            extract(text, 7).as_deref(),
            Some("Adds one.\n\nSecond paragraph.")
        );
    }

    #[test]
    fn a_block_comment_loses_its_markers() {
        let text = "/**\n * Finds a user.\n * @param name the name\n */\nfunction find(name) {}\n";
        assert_eq!(
            extract(text, 5).as_deref(),
            Some("Finds a user.\n@param name the name")
        );
    }

    #[test]
    fn a_python_docstring_is_read_after_a_multi_line_signature() {
        let text = "def login(\n    user,\n    password,\n):\n    \"\"\"Checks the password.\n\n    Raises on failure.\n    \"\"\"\n    return 1\n";
        assert_eq!(
            extract(text, 1).as_deref(),
            Some("Checks the password.\n\nRaises on failure.")
        );
        let one_line = "def f():\n    '''Short.'''\n    pass\n";
        assert_eq!(extract(one_line, 1).as_deref(), Some("Short."));
    }

    #[test]
    fn code_above_a_symbol_is_not_documentation() {
        let text = "x = 1\ndef f():\n    return x\n";
        assert_eq!(extract(text, 2), None);
        assert_eq!(extract("", 1), None);
        assert_eq!(extract(text, 99), None);
    }

    #[test]
    fn an_unclosed_docstring_is_not_guessed_at() {
        assert_eq!(
            extract("def f():\n    \"\"\"never closed\n    pass\n", 1),
            None
        );
    }
}
