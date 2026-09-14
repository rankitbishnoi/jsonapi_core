# Contributing to jsonapi_core

Thanks for your interest in improving `jsonapi_core`. This guide covers how to
build, test, and submit changes.

## Prerequisites

- Rust **1.94.1+** (the crate's MSRV) and the **2024 edition**.
- No other system dependencies — the crate is pure Rust.

## Building and testing

```sh
# Build the whole workspace
cargo build --workspace

# Run the full test suite (unit, integration, doc, and compile-fail tests)
cargo test --workspace --all-features

# The crate must also work with the derive feature off
cargo test -p jsonapi_core --no-default-features

# Benchmarks (Criterion)
cargo bench -p jsonapi_core --bench serde_hot_path
```

Compile-fail tests use [`trybuild`]. If you change the derive macro's
diagnostics, refresh the expected output with `TRYBUILD=overwrite cargo test`.

## Before you open a PR

All of the following must pass — CI enforces them:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p jsonapi_core --no-default-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

New behaviour needs test coverage, and public items need rustdoc — the crate
builds with `#![deny(missing_docs)]`.

## Commit and branch conventions

- Commits follow [Conventional Commits]: `type(scope): description`
  (`feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`, `ci`, …).
  Breaking changes use a `!` (e.g. `refactor!: rename ...`).
- Keep each commit a single logical change so `git bisect` stays useful.
- Branch names: `type/short-description` (e.g. `feat/cursor-pagination`).
- Do not bypass commit hooks with `--no-verify`.

## Versioning

`jsonapi_core` and `jsonapi_core_derive` are versioned in **lockstep** via
`workspace.package.version` and released together. See the
[versioning policy](README.md#versioning-policy) for what counts as public API
and the MSRV policy. Add a `CHANGELOG.md` entry under `[Unreleased]` for any
user-visible change, leading with what a user can now do.

## Reporting security issues

Please do **not** open a public issue for security vulnerabilities — see
[SECURITY.md](SECURITY.md).

[`trybuild`]: https://docs.rs/trybuild
[Conventional Commits]: https://www.conventionalcommits.org/
