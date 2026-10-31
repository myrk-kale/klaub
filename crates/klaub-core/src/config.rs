//! Configuration schema (`.klaub.toml`) and its parsing.
//!
//! Every section has defaults, unknown keys are rejected so typos surface as
//! errors with a line and column, and layers (user, then project) are merged
//! key by key. Reading files is the job of the
//! [`ConfigSource`](crate::ports::ConfigSource) port; this module only parses text.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::semantic::{MatchThresholds, PROBABLE_MATCH, STRONG_MATCH};

/// Which comparison `klaub review` makes by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultScope {
    /// Base against the whole worktree.
    Worktree,
    /// Base against the index.
    Staged,
    /// Index against the worktree.
    Unstaged,
    /// Merge base against `HEAD`.
    Branch,
}

/// How the queue is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// By priority, then score.
    Priority,
    /// By module, then priority.
    Module,
    /// By call-graph order (callers before callees).
    Execution,
}

/// `[review]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReviewConfig {
    /// Base revision (`HEAD`, `main`, a commit).
    pub base: String,
    /// The default comparison.
    pub default_scope: DefaultScope,
    /// Queue order.
    pub sort: SortOrder,
    /// Include untracked, non-ignored files as added files.
    pub include_untracked: bool,
    /// Files larger than this many bytes are listed without content.
    pub max_file_bytes: u64,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            base: "HEAD".to_owned(),
            default_scope: DefaultScope::Worktree,
            sort: SortOrder::Priority,
            include_untracked: true,
            max_file_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Colour themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Dark background.
    Dark,
    /// Light background.
    Light,
    /// Maximum contrast, no reliance on colour alone.
    HighContrast,
}

/// `[ui]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    /// Colour theme.
    pub theme: Theme,
    /// Show key hints in the status bar.
    pub show_hints: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            show_hints: true,
        }
    }
}

/// How feedback is submitted to the agent. Only manual submission exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmitMode {
    /// The user presses `Enter`; Klaub never does.
    Manual,
}

/// `[integration]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IntegrationConfig {
    /// Always `manual` in v1.
    pub submit_mode: SubmitMode,
    /// Ask before pasting into the agent's input.
    pub paste_requires_confirmation: bool,
    /// tmux prefix key, in tmux notation (`C-b`).
    pub tmux_prefix: String,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            submit_mode: SubmitMode::Manual,
            paste_requires_confirmation: true,
            tmux_prefix: "C-b".to_owned(),
        }
    }
}

/// `[analysis]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnalysisConfig {
    /// Enabled languages by name.
    pub languages: Vec<String>,
    /// Deepest caller chain explored.
    pub max_call_depth: u32,
    /// Lower bound of a strong symbol match.
    pub match_strong_threshold: f32,
    /// Lower bound of a probable symbol match.
    pub match_probable_threshold: f32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            languages: ["python", "javascript", "typescript", "rust", "go"]
                .map(str::to_owned)
                .to_vec(),
            max_call_depth: 8,
            match_strong_threshold: STRONG_MATCH,
            match_probable_threshold: PROBABLE_MATCH,
        }
    }
}

impl AnalysisConfig {
    /// The matching thresholds as a value type.
    pub fn thresholds(&self) -> MatchThresholds {
        MatchThresholds {
            strong: self.match_strong_threshold,
            probable: self.match_probable_threshold,
        }
    }
}

/// `[context]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContextConfig {
    /// Optional path of a brief file; never discovered automatically.
    pub brief_file: String,
}

/// Whether tests run by themselves. Only manual execution exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Run only on explicit request, after a preview and confirmation.
    Manual,
}

/// `[tests]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TestsConfig {
    /// Show test evidence.
    pub enabled: bool,
    /// Always `manual` in v1.
    pub run_mode: RunMode,
    /// Report files to read (JUnit XML, Coverage.py XML), relative to the worktree.
    pub report_paths: Vec<String>,
    /// Glob patterns for Python test files.
    pub python_patterns: Vec<String>,
    /// Glob patterns for JavaScript and TypeScript test files.
    pub js_patterns: Vec<String>,
    /// Glob patterns for Rust test files.
    pub rust_patterns: Vec<String>,
    /// Glob patterns for Go test files.
    pub go_patterns: Vec<String>,
    /// The command that runs tests. Optional; never inferred from the repository.
    pub command: Option<Vec<String>>,
    /// Timeout of a test run in seconds.
    pub timeout_secs: u64,
}

impl Default for TestsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            run_mode: RunMode::Manual,
            report_paths: Vec::new(),
            python_patterns: ["tests/test_*.py", "**/test_*.py", "**/*_test.py"]
                .map(str::to_owned)
                .to_vec(),
            js_patterns: [
                "**/*.test.ts",
                "**/*.spec.ts",
                "**/*.test.tsx",
                "**/*.spec.tsx",
                "**/*.test.js",
                "**/*.spec.js",
            ]
            .map(str::to_owned)
            .to_vec(),
            rust_patterns: vec!["tests/**/*.rs".to_owned()],
            go_patterns: vec!["**/*_test.go".to_owned()],
            command: None,
            timeout_secs: 600,
        }
    }
}

/// A dependency that must not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForbiddenDependency {
    /// Directory (module) that must not depend on `to`.
    pub from: String,
    /// Directory (module) that `from` must not depend on.
    pub to: String,
}

/// `[architecture]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArchitectureConfig {
    /// Module roots (directories). Empty means: infer modules from directories.
    pub modules: Vec<String>,
    /// Module pairs where the dependency is forbidden.
    pub forbidden_dependencies: Vec<ForbiddenDependency>,
}

/// `[priority]`: weights and thresholds of the scoring model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PriorityConfig {
    /// Points for a changed contract (signature, visibility, export).
    pub contract_change: i32,
    /// Points when an external entrypoint can reach the change.
    pub entrypoint_external: i32,
    /// Points when only internal entrypoints reach the change.
    pub entrypoint_internal: i32,
    /// Points for high fan-in (many direct callers).
    pub fan_in: i32,
    /// Points when the change crosses a module boundary.
    pub cross_module: i32,
    /// Points when the reach is global.
    pub global_scope: i32,
    /// Points for sensitive areas (authentication, data, migrations).
    pub criticality: i32,
    /// Points for high complexity.
    pub complexity: i32,
    /// Points when complexity grew considerably.
    pub complexity_growth: i32,
    /// Points per technical-risk finding (capped by `risk_cap`).
    pub risk_per_finding: i32,
    /// Points per *critical* finding (injection, secrets, shell and dynamic execution, disabled
    /// certificate verification), which are also what unlocks `P0`.
    pub risk_critical: i32,
    /// Cap for technical-risk points.
    pub risk_cap: i32,
    /// Points when no associated test was found.
    pub no_tests: i32,
    /// Points for an architecture violation.
    pub architecture_violation: i32,
    /// Negative points for formatting-only changes.
    pub format_only: i32,
    /// Negative points for test files.
    pub test_file: i32,
    /// Negative points for generated files.
    pub generated: i32,
    /// Negative points for pure moves.
    pub moved: i32,
    /// Minimum score for P0 (a critical piece of evidence is also required).
    pub p0_threshold: i32,
    /// Minimum score for P1.
    pub p1_threshold: i32,
    /// Minimum score for P2.
    pub p2_threshold: i32,
    /// Direct callers needed to count as high fan-in.
    pub fan_in_threshold: u32,
    /// Cyclomatic complexity considered high.
    pub complexity_threshold: u32,
}

impl Default for PriorityConfig {
    fn default() -> Self {
        Self {
            contract_change: 25,
            entrypoint_external: 20,
            entrypoint_internal: 8,
            fan_in: 10,
            cross_module: 15,
            global_scope: 20,
            criticality: 15,
            complexity: 10,
            complexity_growth: 8,
            risk_per_finding: 15,
            risk_critical: 30,
            risk_cap: 60,
            no_tests: 8,
            architecture_violation: 45,
            format_only: -40,
            test_file: -15,
            generated: -30,
            moved: -15,
            p0_threshold: 70,
            p1_threshold: 45,
            p2_threshold: 20,
            fan_in_threshold: 5,
            complexity_threshold: 15,
        }
    }
}

/// How coverage of the human review is counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountUnit {
    /// Count symbols (and files without symbols).
    Symbol,
    /// Count files.
    File,
}

/// `[coverage]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoverageConfig {
    /// What the progress denominator counts.
    pub count_unit: CountUnit,
    /// Warn (without blocking) when finishing with unreviewed P0/P1 units.
    pub warn_on_unreviewed_p0_p1: bool,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            count_unit: CountUnit::Symbol,
            warn_on_unreviewed_p0_p1: true,
        }
    }
}

/// What to do when no flow diagram exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingPrompt {
    /// Offer to prepare the request in the agent, after confirmation.
    Confirm,
    /// Only show the notice.
    Never,
}

/// `[flows]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FlowsConfig {
    /// Enable the Flow mode.
    pub enabled: bool,
    /// Directory (relative to the worktree) searched for flow documents.
    pub directory: String,
    /// Largest accepted flow document.
    pub max_file_bytes: u64,
    /// Behaviour when a flow is missing.
    pub missing_prompt: MissingPrompt,
}

impl Default for FlowsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: ".klaub/flows".to_owned(),
            max_file_bytes: 1024 * 1024,
            missing_prompt: MissingPrompt::Confirm,
        }
    }
}

/// `[rules]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    /// Rule ids that are turned off (the user's way to silence a false positive).
    pub disabled: Vec<String>,
}

/// The whole configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// `[review]`
    pub review: ReviewConfig,
    /// `[ui]`
    pub ui: UiConfig,
    /// `[integration]`
    pub integration: IntegrationConfig,
    /// `[analysis]`
    pub analysis: AnalysisConfig,
    /// `[context]`
    pub context: ContextConfig,
    /// `[tests]`
    pub tests: TestsConfig,
    /// `[architecture]`
    pub architecture: ArchitectureConfig,
    /// `[priority]`
    pub priority: PriorityConfig,
    /// `[coverage]`
    pub coverage: CoverageConfig,
    /// `[flows]`
    pub flows: FlowsConfig,
    /// `[rules]`
    pub rules: RulesConfig,
}

impl Config {
    /// Parses one configuration text. `origin` names it in error messages.
    pub fn parse(origin: &str, content: &str) -> Result<Self> {
        toml::from_str(content).map_err(|e| config_error(origin, content, &e))
    }

    /// Parses and merges layers, lowest priority first (user, then project).
    ///
    /// Tables are merged key by key; scalars and arrays of a later layer replace
    /// earlier ones. Each layer is validated on its own first, so errors point at
    /// the file and position that caused them.
    pub fn from_layers(layers: &[(&str, &str)]) -> Result<Self> {
        let mut merged = toml::Table::new();
        for (origin, content) in layers {
            Self::parse(origin, content)?;
            let table: toml::Table =
                toml::from_str(content).map_err(|e| config_error(origin, content, &e))?;
            merge_tables(&mut merged, table);
        }
        merged
            .try_into()
            .map_err(|e: toml::de::Error| Error::Config {
                location: "merged configuration".to_owned(),
                message: e.message().to_owned(),
            })
    }

    /// Checks cross-field invariants that the schema cannot express.
    pub fn validate(&self) -> Result<()> {
        let a = &self.analysis;
        let in_unit = |v: f32| (0.0..=1.0).contains(&v);
        if !in_unit(a.match_strong_threshold) || !in_unit(a.match_probable_threshold) {
            return Err(config_value(
                "analysis.match_*_threshold",
                "must be between 0 and 1",
            ));
        }
        if a.match_probable_threshold > a.match_strong_threshold {
            return Err(config_value(
                "analysis.match_probable_threshold",
                "must not exceed match_strong_threshold",
            ));
        }
        if a.max_call_depth == 0 || a.max_call_depth > 64 {
            return Err(config_value(
                "analysis.max_call_depth",
                "must be between 1 and 64",
            ));
        }
        let p = &self.priority;
        if !(p.p0_threshold >= p.p1_threshold && p.p1_threshold >= p.p2_threshold) {
            return Err(config_value(
                "priority.*_threshold",
                "must satisfy p0 >= p1 >= p2",
            ));
        }
        if let Some(command) = &self.tests.command
            && command
                .first()
                .is_none_or(|program| program.trim().is_empty())
        {
            return Err(config_value("tests.command", "must name an executable"));
        }
        if self.flows.directory.starts_with('/')
            || self.flows.directory.split('/').any(|c| c == "..")
        {
            return Err(config_value(
                "flows.directory",
                "must be a relative path inside the worktree",
            ));
        }
        if self.integration.tmux_prefix.trim().is_empty() {
            return Err(config_value("integration.tmux_prefix", "must not be empty"));
        }
        Ok(())
    }
}

fn config_value(key: &str, message: &str) -> Error {
    Error::Config {
        location: key.to_owned(),
        message: message.to_owned(),
    }
}

fn merge_tables(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(existing)), toml::Value::Table(incoming)) => {
                merge_tables(existing, incoming);
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn config_error(origin: &str, content: &str, error: &toml::de::Error) -> Error {
    let location = match error.span() {
        Some(span) => {
            let before = &content[..span.start.min(content.len())];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
            format!("{origin}:{line}:{column}")
        }
        None => origin.to_owned(),
    };
    Error::Config {
        location,
        message: error.message().trim().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_EXAMPLE: &str = r#"
[review]
base = "HEAD"
default_scope = "worktree"
sort = "priority"
include_untracked = true

[ui]
theme = "dark"
show_hints = true

[integration]
submit_mode = "manual"
paste_requires_confirmation = true
tmux_prefix = "C-b"

[analysis]
languages = ["python", "javascript", "typescript"]
max_call_depth = 8
match_strong_threshold = 0.85
match_probable_threshold = 0.65

[context]
brief_file = ""

[tests]
enabled = true
run_mode = "manual"
report_paths = []
python_patterns = ["tests/test_*.py", "**/test_*.py"]
js_patterns = ["**/*.test.ts", "**/*.spec.ts", "**/*.test.js", "**/*.spec.js"]

[architecture]
modules = ["src/api", "src/domain", "src/infra"]

[[architecture.forbidden_dependencies]]
from = "src/domain"
to = "src/infra"

[priority]
contract_change = 25
cross_module = 15

[coverage]
count_unit = "symbol"
warn_on_unreviewed_p0_p1 = true

[flows]
enabled = true
directory = ".klaub/flows"
max_file_bytes = 1048576
missing_prompt = "confirm"
"#;

    #[test]
    fn the_specification_example_parses() {
        let config = Config::parse("spec", SPEC_EXAMPLE).unwrap();
        config.validate().unwrap();
        assert_eq!(
            config.architecture.forbidden_dependencies[0].from,
            "src/domain"
        );
        assert_eq!(config.analysis.languages.len(), 3);
        assert_eq!(config.priority.contract_change, 25);
    }

    #[test]
    fn empty_input_yields_defaults() {
        let config = Config::parse("empty", "").unwrap();
        assert_eq!(config, Config::default());
        config.validate().unwrap();
        assert_eq!(config.integration.tmux_prefix, "C-b");
    }

    #[test]
    fn typos_report_line_and_column() {
        let text = "[review]\nbase = \"HEAD\"\nunknown_key = \"main\"\n";
        let error = Config::parse(".klaub.toml", text).unwrap_err().to_string();
        assert!(error.contains(".klaub.toml:3:1"), "{error}");
        assert!(error.contains("unknown_key"), "{error}");
    }

    #[test]
    fn invalid_enum_values_are_rejected() {
        assert!(Config::parse("x", "[integration]\nsubmit_mode = \"auto\"\n").is_err());
        assert!(Config::parse("x", "[tests]\nrun_mode = \"auto\"\n").is_err());
    }

    #[test]
    fn layers_merge_key_by_key() {
        let user = "[ui]\ntheme = \"light\"\nshow_hints = false\n[review]\nbase = \"main\"\n";
        let project = "[ui]\ntheme = \"high_contrast\"\n";
        let config = Config::from_layers(&[("user", user), ("project", project)]).unwrap();
        assert_eq!(config.ui.theme, Theme::HighContrast);
        assert!(
            !config.ui.show_hints,
            "keys absent from the project layer survive"
        );
        assert_eq!(config.review.base, "main");
    }

    #[test]
    fn a_broken_layer_is_named_in_the_error() {
        let error =
            Config::from_layers(&[("user", ""), ("project", "[ui]\ntheme = 3\n")]).unwrap_err();
        assert!(error.to_string().contains("project:2:"), "{error}");
    }

    #[test]
    fn validation_catches_inconsistent_values() {
        let mut config = Config::default();
        config.analysis.match_probable_threshold = 0.9;
        assert!(config.validate().is_err());

        let mut config = Config::default();
        config.flows.directory = "../outside".to_owned();
        assert!(config.validate().is_err());

        let mut config = Config::default();
        config.tests.command = Some(vec![]);
        assert!(config.validate().is_err());

        let mut config = Config::default();
        config.priority.p1_threshold = 90;
        assert!(config.validate().is_err());
    }
}
