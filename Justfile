package := "rust-chess"
rustfmt := "rustfmt --edition 2024 --style-edition 2024"
import "common.just"

build: common-build

check: common-check

depcheck: common-depcheck

fmt: common-fmt
    just --justfile common.just --fmt
    {{ rustfmt }} scripts/{polyglot-basis,slider-sights,standard-basis}.rs

install: common-install

lint: common-lint
    just --justfile common.just --fmt --check
    {{ rustfmt }} --check scripts/{polyglot-basis,slider-sights,standard-basis}.rs

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
    RUSTFLAGS="--cfg rust_chess_compile_time_slider_sights" rust-script scripts/slider-sights.rs

standard-basis:
    RUSTFLAGS="--cfg rust_chess_compile_time_standard_basis" rust-script scripts/standard-basis.rs

polyglot-basis:
    RUSTFLAGS="--cfg rust_chess_compile_time_polyglot_basis" rust-script scripts/polyglot-basis.rs

clean:
    rm -f *.cb{a,c,e,g,h,j,l,m,p,s,t,tt,ini} *.{flags,ini}

perft:
    cargo nextest run --release perft

gossimit := "http://web.archive.org/web/20060710121413/http://www.uni-klu.ac.at/~gossimit/c"
theweekinchess := "https://theweekinchess.com/zips"

twic:
    mkdir -p twic
    wget -qrP twic {{ theweekinchess }}/twic16{16,32}{g,c6}.zip
    curl -fLso twic/magictal.zip {{ gossimit }}/magictal.zip
    curl -fLso twic/best_less.zip {{ gossimit }}/best_less.zip
    unzip -oqqd twic 'twic/*.zip'
    cargo run --release -qp rust-chess-bin -- convert -i twic/twic1616.cbv -o twic cb
    cargo run --release -qp rust-chess-bin -- convert -i twic/twic1632.cbv -o twic cb
    cargo run --release -qp rust-chess-bin -- convert -i twic/best_less.cbv -o twic cb
