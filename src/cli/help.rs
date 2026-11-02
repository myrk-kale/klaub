//! `--help` in the user's language.
//!
//! The derived grammar in [`crate::cli::args`] carries the English text as doc comments. For any
//! other language this module walks the finished command tree and replaces the descriptions with
//! the ones in the catalog. English is left exactly as clap derived it, and the tests keep the
//! catalog's English entries equal to those doc comments.

use clap::{Arg, Command};

use crate::domain::i18n::{Locale, cli as t};

/// `command` with its help text in `locale`. English is returned untouched.
pub(crate) fn localized(command: Command, locale: Locale) -> Command {
    if locale == Locale::En {
        return command;
    }
    // Building adds the `help` subcommand and the `-h` and `-V` flags, which are translated too.
    let mut command = command;
    command.build();
    localize(command, "", locale)
}

fn localize(command: Command, path: &str, locale: Locale) -> Command {
    let here = if path.is_empty() {
        String::new()
    } else {
        path.to_owned()
    };
    let key = |suffix: &str| match (here.is_empty(), suffix.is_empty()) {
        (true, true) => String::new(),
        (true, false) => suffix.to_owned(),
        (false, true) => here.clone(),
        (false, false) => format!("{here}.{suffix}"),
    };

    let mut command = command
        .subcommand_help_heading(t::help_commands(locale))
        .help_template(template(locale))
        .mut_args(|arg| localize_arg(arg, &key(""), locale));
    if let Some(about) = text(&key(""), locale) {
        command = command.about(without_final_period(about));
    }
    if let Some(long) = text(&key("long"), locale) {
        command = command.long_about(long);
    }
    if let Some(after) = text(&key("after"), locale) {
        command = command.after_help(after);
    }
    if command.find_subcommand("help").is_some() {
        command = command.mut_subcommand("help", |help| help.about(t::help_help_command(locale)));
    }
    command.mut_subcommands(|sub| {
        let child = if here.is_empty() {
            sub.get_name().to_owned()
        } else {
            format!("{here}.{}", sub.get_name())
        };
        localize(sub, &child, locale)
    })
}

fn localize_arg(arg: Arg, path: &str, locale: Locale) -> Arg {
    let id = arg.get_id().as_str().to_owned();
    let heading = if arg.is_positional() {
        t::help_arguments(locale)
    } else {
        t::help_options(locale)
    };
    let arg = arg.help_heading(heading);
    match id.as_str() {
        "help" => {
            return arg
                .help(t::help_print_help(locale))
                .long_help(t::help_print_help(locale));
        }
        "version" => {
            return arg
                .help(t::help_print_version(locale))
                .long_help(t::help_print_version(locale));
        }
        _ => {}
    }
    // A global option such as `--tmux-path` is repeated in every subcommand under the same id, so
    // the root's text applies where the subcommand has none of its own.
    let own = (!path.is_empty()).then(|| format!("{path}.{id}"));
    match own
        .as_deref()
        .and_then(|key| text(key, locale))
        .or_else(|| text(&id, locale))
    {
        Some(help) => arg.help(without_final_period(help)),
        None => arg,
    }
}

/// clap drops the final period of a one-paragraph description in its short help; the catalog keeps
/// it, so the translated text looks like the derived one.
fn without_final_period(text: &str) -> &str {
    match text.strip_suffix('.') {
        Some(rest) if !text.contains("\n\n") && !rest.ends_with('.') => rest,
        _ => text,
    }
}

/// clap's default layout, with the word `Usage` in the user's language.
fn template(locale: Locale) -> String {
    format!(
        "{{before-help}}{{about-with-newline}}\n{}: {{usage}}\n\n{{all-args}}{{after-help}}",
        t::help_usage(locale)
    )
}

/// The catalog text for a command (`wrap`, `reviews.export`, `""` for the root) or one of its
/// arguments (`wrap.detach`). `None` for what the catalog has nothing to say about, which then
/// keeps clap's own text.
pub(crate) fn text(key: &str, locale: Locale) -> Option<&'static str> {
    Some(match key {
        "" => t::help_root(locale),
        "long" => t::help_root_long(locale),
        "tmux_path" => t::help_tmux_path(locale),
        "wrap" => t::help_wrap(locale),
        "wrap.after" => t::help_wrap_after(locale),
        "wrap.detach" => t::help_wrap_detach(locale),
        "wrap.prefix" => t::help_wrap_prefix(locale),
        "wrap.command" => t::help_wrap_command(locale),
        "review" => t::help_review(locale),
        "review.session" => t::help_review_session(locale),
        "review.base" => t::help_review_base(locale),
        "review.scope" => t::help_review_scope(locale),
        "review.brief" => t::help_review_brief(locale),
        "review.wait" => t::help_review_wait(locale),
        "review.format" => t::help_review_format(locale),
        "reviews" => t::help_reviews(locale),
        "reviews.list" => t::help_reviews_list(locale),
        "reviews.show" => t::help_reviews_show(locale),
        "reviews.export" => t::help_reviews_export(locale),
        "reviews.export.format" => t::help_export_format(locale),
        "reviews.show.id" | "reviews.export.id" | "feedback.id" | "tests.list.review"
        | "tests.run.review" => t::help_review_id(locale),
        "doctor.format"
        | "version.format"
        | "reviews.list.format"
        | "reviews.show.format"
        | "session.list.format"
        | "crashes.list.format"
        | "feedback.format" => t::help_format(locale),
        "feedback" => t::help_feedback(locale),
        "tests" => t::help_tests(locale),
        "tests.list" => t::help_tests_list(locale),
        "tests.run" => t::help_tests_run(locale),
        "tests.run.select" => t::help_tests_select(locale),
        "tests.run.yes" | "session.kill.yes" => t::help_yes(locale),
        "flow" => t::help_flow(locale),
        "flow.status" => t::help_flow_status(locale),
        "flow.context" => t::help_flow_context(locale),
        "flow.validate" => t::help_flow_validate(locale),
        "flow.validate.file" => t::help_flow_file(locale),
        "flow.prompt" => t::help_flow_prompt(locale),
        "session" => t::help_session(locale),
        "session.list" => t::help_session_list(locale),
        "session.attach" => t::help_session_attach(locale),
        "session.attach.id" => t::help_session_attach_id(locale),
        "session.kill" => t::help_session_kill(locale),
        "session.kill.id" => t::help_session_kill_id(locale),
        "doctor" => t::help_doctor(locale),
        "crashes" => t::help_crashes(locale),
        "crashes.list" => t::help_crashes_list(locale),
        "crashes.show" => t::help_crashes_show(locale),
        "crashes.show.name" => t::help_crashes_name(locale),
        "version" => t::help_version(locale),
        "completions" => t::help_completions(locale),
        "completions.shell" => t::help_completions_shell(locale),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::Cli;
    use clap::CommandFactory;

    /// Every visible command and argument of the grammar, with the key the catalog uses.
    fn keys(command: &Command, path: &str, out: &mut Vec<(String, Option<String>)>) {
        for arg in command.get_arguments() {
            let id = arg.get_id().as_str();
            if arg.is_hide_set() || matches!(id, "help" | "version") {
                continue;
            }
            let key = if path.is_empty() {
                id.to_owned()
            } else {
                format!("{path}.{id}")
            };
            out.push((key, arg.get_help().map(ToString::to_string)));
        }
        for sub in command.get_subcommands() {
            if sub.is_hide_set() || sub.get_name() == "help" {
                continue;
            }
            let child = if path.is_empty() {
                sub.get_name().to_owned()
            } else {
                format!("{path}.{}", sub.get_name())
            };
            out.push((child.clone(), sub.get_about().map(ToString::to_string)));
            keys(sub, &child, out);
        }
    }

    fn grammar() -> Vec<(String, Option<String>)> {
        let mut out = vec![(
            String::new(),
            Cli::command().get_about().map(ToString::to_string),
        )];
        keys(&Cli::command(), "", &mut out);
        out
    }

    #[test]
    fn every_visible_command_and_argument_has_a_translation() {
        for locale in Locale::ALL {
            for (key, _) in grammar() {
                assert!(
                    text(&key, locale).is_some(),
                    "`{key}` has no help text in {locale:?}"
                );
            }
        }
    }

    #[test]
    fn the_english_entries_equal_the_doc_comments_clap_derives() {
        for (key, derived) in grammar() {
            assert_eq!(
                text(&key, Locale::En).map(without_final_period),
                derived.as_deref().map(without_final_period),
                "the English catalog entry for `{key}` differs from the doc comment"
            );
        }
        let command = Cli::command();
        assert_eq!(
            text("long", Locale::En),
            command.get_long_about().map(|s| s.to_string()).as_deref()
        );
        let wrap = command.find_subcommand("wrap").unwrap();
        assert_eq!(
            text("wrap.after", Locale::En),
            wrap.get_after_help().map(|s| s.to_string()).as_deref()
        );
    }

    #[test]
    fn english_is_left_exactly_as_clap_derived_it() {
        let plain = Cli::command().render_help().to_string();
        let english = localized(Cli::command(), Locale::En)
            .render_help()
            .to_string();
        assert_eq!(plain, english);
    }

    #[test]
    fn a_translated_help_names_the_commands_in_the_language() {
        let mut portuguese = localized(Cli::command(), Locale::Pt);
        let root = portuguese.render_help().to_string();
        assert!(root.contains("Uso:"), "{root}");
        assert!(root.contains("Comandos:"), "{root}");
        assert!(root.contains("Opções:"), "{root}");
        assert!(root.contains("Abre o revisor"), "{root}");
        assert!(root.contains("Mostra a ajuda"), "{root}");
        assert!(!root.contains("Open the reviewer"), "{root}");

        let wrap = portuguese
            .find_subcommand_mut("wrap")
            .unwrap()
            .render_help()
            .to_string();
        assert!(wrap.contains("Argumentos:"), "{wrap}");
        assert!(wrap.contains("Exemplos:"), "{wrap}");
        assert!(wrap.contains("Inicia a sessão sem anexar a ela"), "{wrap}");

        let mut spanish = localized(Cli::command(), Locale::Es);
        let root = spanish.render_help().to_string();
        assert!(root.contains("Abre el revisor"), "{root}");
    }

    #[test]
    fn a_translated_grammar_still_parses() {
        let command = localized(Cli::command(), Locale::Es);
        command.clone().debug_assert();
        let matches = command
            .try_get_matches_from(["klaub", "wrap", "--detach", "--", "codex"])
            .unwrap();
        assert_eq!(matches.subcommand_name(), Some("wrap"));
    }
}
