set shell := ["bash", "-c"]

binary := if package == "workspace" { "" } else if path_exists("bin/Cargo.toml") == "true" { package + "-bin" } else { "" }

cargo_spec := if package == "workspace" { "" } else { "--package " + package + if binary != "" { " --package " + binary } else { "" } }

common-build:
    cargo build {{ cargo_spec }} --tests

common-check:
    cargo check {{ cargo_spec }} --tests

common-depcheck:
    #!/usr/bin/env bash
    status=0
    cargo shear || status=$?
    cargo upgrade --dry-run --incompatible || status=$?
    exit "$status"

common-fmt:
    just --fmt
    cargo fmt {{ cargo_spec }}
    cargo clippy {{ cargo_spec }} --tests --fix --allow-dirty --allow-staged

common-install:
    {{ if binary != "" { "cargo install --frozen --path bin" } else { ":" } }}

common-lint:
    just --fmt --check
    cargo fmt --check {{ cargo_spec }}
    cargo clippy {{ cargo_spec }} --tests -- -D warnings

common-test:
    cargo test {{ cargo_spec }} --doc
    cargo nextest run {{ cargo_spec }}

common-uninstall:
    {{ if binary != "" { "cargo uninstall " + binary } else { ":" } }}

common-update:
    cargo update
