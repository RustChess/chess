package := "rust-chess"
import "common.just"

build: common-build

check: common-check

depcheck: common-depcheck

fmt: common-fmt
    just --justfile common.just --fmt
    rustfmt scripts/{polyglot,scharnagl,standard}-id.rs
    rustfmt src/id/{polyglot,standard}.rs
    rustfmt src/board/scharnagl-id.rs

install: common-install

lint: common-lint
    just --justfile common.just --fmt --check
    rustfmt --check scripts/{polyglot,scharnagl,standard}-id.rs
    rustfmt --check src/id/{polyglot,standard}.rs
    rustfmt --check src/board/scharnagl-id.rs

test: common-test

uninstall: common-uninstall

update: common-update

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
