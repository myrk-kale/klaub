# Languages

Klaub speaks English, Portuguese (Brazil) and Spanish. The reader, its prompts and messages, `klaub doctor`, the other commands' diagnostics and `--help` follow the language you choose.

## Choosing a language

The first of these that names a language Klaub speaks decides:

1. `ui.language` in your user configuration (`~/.config/klaub/config.toml`), when it is not `auto`.
2. `KLAUB_LANG`, for example `KLAUB_LANG=pt`.
3. The POSIX locale variables, in their usual order: `LC_ALL`, `LC_MESSAGES`, `LANG`. The first one that is set and not empty decides, so `LC_ALL=fr_FR` gives English even when `LANG=pt_BR.UTF-8`.
4. English.

Only the language part of a tag counts: `pt`, `pt_BR`, `pt_BR.UTF-8` and `pt-PT` all mean Portuguese; `es`, `es_MX` and `es_ES.UTF-8` all mean Spanish.

`klaub review` also reads `ui.language` from the project's `.klaub.toml`, which wins over the user configuration for that project:

```toml
[ui]
language = "pt"   # "auto" (the default), "en", "pt" or "es"
```

## What is translated

| Translated | Stays in English |
| --- | --- |
| Every screen, hint, prompt and status message of the reader, the command palette and the help | The findings, their evidence and the reasons behind a priority (they come from the analysis rules) |
| Messages and prompts of `klaub review`, `reviews`, `session`, `tests`, `crashes`, `version`, `wrap` and `doctor` | Everything machine-readable: JSON output (including the `detail` of `klaub doctor --format json`), review documents, Markdown and SARIF exports |
| `klaub --help` and `klaub <command> --help` | The feedback and flow requests Klaub pastes into the agent, and the text of errors that come from Git, tmux or the file system |
| Yes/no prompts: `s`, `sim` (Portuguese) and `s`, `si`, `sí` (Spanish) are accepted next to `y` and `yes` | clap's own words in the usage line (`[OPTIONS]`, `<COMMAND>`), `[default: ...]` and its parse errors |

Keys do not change with the language. The reader's hints name the same keys in every language.

## How it is built

There are no translation files to parse at run time. Every sentence is a function in `src/domain/i18n/tui.rs` (the reader) or `src/domain/i18n/cli.rs` (the command line), written next to its translations:

```rust
messages! {
    fn marked_reviewed(done: usize, total: usize) {
        en: "Marked reviewed ({done}/{total}). Press r again to undo, n for the next.",
        pt: "Marcada como revisada ({done}/{total}). Pressione r de novo para desfazer, n para a próxima.",
        es: "Marcada como revisada ({done}/{total}). Pulsa r de nuevo para deshacer, n para la siguiente.",
    }
}
```

`texts!` does the same for a sentence with no values and returns a `&'static str`. The caller passes the `Locale` first:

```rust
app.say(StatusKind::Info, t::marked_reviewed(app.locale, c.reviewed, c.total));
```

What this gives you:

- An entry without a language does not compile. A placeholder that is not an argument does not compile either, because the text is a `format!` string.
- `tests/i18n.rs` covers what the compiler cannot see: a translation that leaves out a placeholder, or that adds or drops the spaces around the text (some sentences are joined to others or aligned in columns).
- `cli::help` keeps the English `--help` entries equal to the doc comments clap derives them from, and a test fails when a command or option has no translation.
- The domain stays free of I/O. Reading `KLAUB_LANG` and the locale variables happens in `cli::env`, the composition root, and the reader receives a concrete `Locale`.

## Adding a sentence

1. Add the entry to `tui.rs` or `cli.rs` with all three languages.
2. Call it with the `Locale` from `app.locale` (reader) or `ctx.locale` (command line).
3. Keep text meant for machines out of the catalog. If a sentence ends up in JSON or is pasted into the agent, leave it in English where it is built.

Words for an enum (`QueueTag`, `ChangeKind`...) go through `src/tui/labels.rs`. The `label()` methods on the domain types stay in English because exports use them.

Some words sit in columns whose width is fixed (the status of a comment, the kind of change). Keep the translation within the width in `views.rs`, or widen the column there.

## Adding a language

1. Add a variant to `Locale` in `src/domain/i18n/mod.rs` (`ALL`, `code`), a variant to `Language` in `src/domain/config.rs`, and a line for it in the two `match`es of the `messages!` and `texts!` macros. The compiler then points at every entry that still needs a translation.
2. Translate `tui.rs`, `cli.rs` and the `--help` entries in `cli.rs`.
3. Run `cargo test`: `tests/i18n.rs` and the reader tests draw every screen in every language at a small and a large terminal size.
4. Add the language to `docs/configuration.md` and to this page.
