{ pkgs ? import <nixpkgs> {} }:
let
  dev_used_stdenv = pkgs.clang19Stdenv;
in
dev_used_stdenv.mkDerivation {
  name = "dev-build-environment";
  buildInputs = with pkgs;
    [
      cargo
      rustc
      clippy
      rust-analyzer
      rustfmt
    ];
}
