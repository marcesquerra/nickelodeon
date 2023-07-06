{ system, packages, rustPackage, rust-src, cargo-watch, bacon, rust-analyzer, nickel, topiary } :
let
  git = "${packages.git}/bin/git";
  cargo-next-bin = packages.writeShellScriptBin "cargo-next-bin" ''
    ${rustPackage}/bin/cargo release --sign "$@" patch
  '';
  cargo-next = packages.writeShellScriptBin "cargo-next" ''
    ${cargo-next-bin}/bin/cargo-next-bin
  '';
  cargo-next-go = packages.writeShellScriptBin "cargo-next-go" ''
    ${cargo-next-bin}/bin/cargo-next-bin --no-confirm --execute
  '';
in
  packages.mkShell {
    name = "Nickelodeon Flake Shell";
    nativeBuildInputs = [
      rustPackage
      cargo-watch
      packages.cargo-release
      cargo-next
      cargo-next-go
      bacon
      rust-analyzer
      nickel.packages.${system}.default
      nickel.packages.${system}.lsp-nls
      topiary.packages.${system}.default
    ];
    shellHook = ''
      export RUST_SRC_PATH="${rust-src}/lib/rustlib/src/rust/library"
    '';
  }