{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs;
    [
      cargo
      rustc
      clippy
      rust-analyzer
      rustfmt

      # Needed for a bindgen dependency in nokhwa
      rustPlatform.bindgenHook

      # Graphing outputs.
      gnuplot
      gawk
    ];

}
