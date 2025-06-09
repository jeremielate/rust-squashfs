{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
	rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, utils, naersk, rust-overlay }:
    utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        naersk-lib = pkgs.callPackage naersk { };
        nightlyRust = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
			extensions = [ "rust-src" "rust-analyzer"];
        });
        rustPlatform = pkgs.makeRustPlatform {
          cargo = nightlyRust;
          rustc = nightlyRust;
        };
      in
      {
        defaultPackage = naersk-lib.buildPackage ./.;
        devShell = pkgs.mkShell {
          nativeBuildInputs = [
            nightlyRust
          ];
          # RUST_SRC_PATH = "${nightlyRust}/lib/rustlib/src/rust/library";
          RUSTDOCFLAGS = "--enable-index-page -Zunstable-options";
        };
      }
    );
}
