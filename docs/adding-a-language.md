# Adding a language

Klaub's analysis is deterministic and offline, so a language is supported by a tree-sitter grammar and a small description of how that grammar names things. Python, JavaScript, TypeScript (and TSX), Rust, Go and Java are in. Java is the most recent one, so its change is the best example of the steps below: `crates/klaub-parse/src/java.rs`, `crates/klaub-parse/tests/java.rs`, the import resolution in `crates/klaub-graph/src/resolve.rs` and the end-to-end test at the end of `crates/klaub-app/tests/session.rs`. This is the list of places a new language touches, in the order a change goes through them. Every step has a test that already exists for another language to copy.

A language you do not add is not a hole: files in it are listed with their metadata, shown whole in the reader and compared as text, and the review says there is no semantic analysis for them. Nothing pretends otherwise.

## 1. The grammar

Add the `tree-sitter-<name>` crate to `[workspace.dependencies]` in `Cargo.toml` and to `crates/klaub-parse/Cargo.toml`. Check that the crate builds on the MSRV and on both release targets (`cargo-deny` also checks its licence, and `THIRD-PARTY-NOTICES.md` lists what ships).

## 2. The enum

In `crates/klaub-core/src/language.rs`:

- add the variant to `Language` and to `Language::ALL`;
- map its file extensions in `from_extension`, and its configuration name in `name` / `from_name`;
- extend the test in that file with a path of the new language.

`klaub doctor` iterates `Language::ALL` and asks the parser for a sample of each. Add a one-function sample for the new language in `grammars_check` (`crates/klaub-cli/src/commands/doctor.rs`); until you do, the compiler's exhaustive `match` there will refuse to build.

## 3. The profile

In `crates/klaub-parse/src/`, add `<name>.rs` implementing the `Profile` trait from `engine.rs`, and select it in `profile_for` in `lib.rs`. The profile answers, for a tree-sitter node:

| Question | Method |
| --- | --- |
| What defines a symbol, and what are its name, kind, signature, visibility and decorators? | `definition` |
| What is a comment, a loop, a nesting level, a decision point (for complexity)? | `is_comment`, `is_loop`, `is_nesting`, `decision_points` |
| What is a call, and what is it called on? | `call` |
| What does an import name? | `imports` |
| Which risk markers are visible on one node, or on a call? | `node_markers`, `call_markers` |
| Does a `raise`/`throw` name an exception? | `raised` (optional) |
| Which symbols are entrypoints? | `entrypoints` |
| How is a token highlighted? | `highlight` |

Start from the profile whose language is closest. Every answer is a statement about syntax. If the grammar cannot tell, return nothing: the pipeline treats a missing fact as unknown, never as zero risk.

Bump `PARSER_VERSION` in `engine.rs`, because cached parses are keyed by it.

## 4. The call graph

`crates/klaub-graph/src/resolve.rs` turns an import into the project file it names (`Language::Go => self.go(...)`). Add the language's module system there. Without it, imports of the new language resolve to nothing and calls across files stay `unresolved`, which the impact view reports honestly but which makes it much less useful. Entrypoint conventions (a `main`, a route decorator, a package manifest) belong in the profile's `entrypoints`.

## 5. Tests that map tests to code

`TestsConfig` has a `<name>_patterns` list per language (`crates/klaub-core/src/config.rs`). Add the new language's default test-file globs, document them in `docs/configuration.md`, and teach `crates/klaub-tests` how a test file of that language names what it tests.

## 6. The tests of the profile

Copy `crates/klaub-parse/tests/python.rs`, `rust_and_go.rs` or `java.rs`: symbols and signatures, nested symbols not changing the fingerprint of their parent, calls with receivers, imports, entrypoints, markers, and a file with a syntax error (the parser is fault tolerant and must still return what it can).

## 7. Where else the language shows up

`cargo build` finds most of these for you, because the matches over `Language` are exhaustive: the signature splitter in `crates/klaub-app/src/signature.rs` (where the return type sits in a declaration), the contract's notion of a data type and of declared errors in `crates/klaub-app/src/contract.rs`, and the import resolver. The default `analysis.languages` list and `tests.<name>_patterns` are in `crates/klaub-core/src/config.rs`, and `is_test_path` in `crates/klaub-rules/src/lib.rs` reads the patterns. If test files are named after the class under test, as Java's `CartTest`, teach `subject_stem` in `crates/klaub-tests/src/lib.rs`.

## 8. What to say in the changelog

Which constructs are recognised and, as important, which are not (macros, reflection, dynamic dispatch), so nobody reads `resolved` as more than it is.
