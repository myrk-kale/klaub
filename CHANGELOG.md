# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `klaub-core`: the I/O-free domain model (snapshots, symbols, semantic changes, findings, reviews, test evidence, flow contract), the configuration schema with line/column errors and layered merging, terminal-text sanitisation, and the ports every adapter implements.
- Executable architecture rules (`crates/klaub-cli/tests/architecture.rs`): dependency direction between layers and an I/O-free core are enforced in CI.
- Project foundation: Cargo workspace, lint policy, continuous integration, security scanning, community files, architecture documentation and decision records.
