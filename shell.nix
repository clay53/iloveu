{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell rec {
    buildInputs = with pkgs; [
        rustup
        # gcc
        # cmake
        pkg-config
        trunk
    ];
    nativeBuildInputs = with pkgs; [
        openssl
    ];
    # RUSTC_VERSION = pkgs.lib.readFile ./rust-toolchain;
    # https://github.com/rust-lang/rust-bindgen#environment-variables
    # LIBCLANG_PATH = pkgs.lib.makeLibraryPath [ pkgs.llvmPackages_latest.libclang.lib ];
    HISTFILE = toString ./.history;
    shellHook = ''
        rustup override set nightly
    '';
}