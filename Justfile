set shell := ["bash", "-c"]

fmt:
    cargo fmt --all
    rustfmt scripts/{polyglot,scharnagl,standard}-id.rs
    rustfmt src/id/{polyglot,standard}.rs
    rustfmt src/board/scharnagl-id.rs
    just --fmt --unstable

lint:
    cargo fmt --check --all
    just --fmt --check --unstable
    cargo clippy --all --tests -- -D warnings

lint-more: lint
    cargo shear
    cargo upgrades

test:
    # TODO: figure out how to disable the docstests for the generated lichess crate
    # cargo test --doc --workspace
    cargo nextest run --workspace

recommit:
    git add -u && git commit --amend --no-edit

freestyle-positions:
    cargo run --example freestyle-positions 2>/dev/null > freestyle-positions.txt

slider-sights:
    rust-script scripts/slider-sights.rs

standard-id:
    rust-script scripts/standard-id.rs
    rustfmt src/id/standard.rs

polyglot-id:
    rust-script scripts/polyglot-id.rs
    rustfmt src/id/polyglot.rs

scharnagl-id:
    rust-script scripts/scharnagl-id.rs
    rustfmt src/board/scharnagl-id.rs

clean:
    rm -f *.cb{a,c,e,g,h,j,l,m,p,s,t,tt,ini} *.{flags,ini}

perft:
    cargo nextest run --release perft

# takes about 30 seconds
deep-perft:
    cargo nextest run --release perft -- --ignored
