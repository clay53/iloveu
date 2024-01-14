let
    pkgs = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/refs/tags/23.05.tar.gz") {
        overlays = [
            (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
        ];
    };
    rust-bin = pkgs.rust-bin.nightly."2023-06-15".minimal.override {
        targets = [ "wasm32-unknown-unknown" ];
    };
    rustPlatform = pkgs.makeRustPlatform {
        cargo = rust-bin;
        rustc = rust-bin;
    };
in
{
    server = rustPlatform.buildRustPackage {
        pname = "iloveu-server";
        version = "1.0";
        src = ./.;

        cargoLock = {
            lockFile = ./Cargo.lock;
        };

        cargoBuildFlags = "--bin iloveu-server";

        doCheck = false;
    };
    yew = rustPlatform.buildRustPackage {
        pname = "iloveu-yew";
        version = "1.0";
        src = ./.;

        cargoLock = {
            lockFile = ./Cargo.lock;
        };

        nativeBuildInputs = [
            pkgs.trunk
            pkgs.wasm-bindgen-cli
        ];

        buildPhase = ''
            cd ./iloveu-yew
            export XDG_CACHE_HOME="$(mktemp -d)"
            API_ROOT="https://iloveu.claytonhickey.me" trunk build --release --dist $out
            rm $XDG_CACHE_HOME -r
        '';

        doCheck = false;

        installPhase = ''echo "help"'';
    };
}
