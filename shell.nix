{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-25.11.tar.gz") {} }:
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
