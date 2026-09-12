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

deep-test *tests:
    cargo nextest run --release {{ cargo_spec }} --run-ignored ignored-only {{ tests }}

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

twic:
    mkdir -p twic
    wget -qrP twic https://theweekinchess.com/zips/twic16{16,32}{g,c6}.zip
    unzip -oqqd twic 'twic/twic*.zip'
    cargo run --release -qp rust-chess-bin -- convert -i twic/twic1616.cbv -o twic cb
    cargo run --release -qp rust-chess-bin -- convert -i twic/twic1632.cbv -o twic cb
