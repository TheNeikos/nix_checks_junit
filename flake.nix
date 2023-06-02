{
  description = "The nix_checks_junit binary";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.05";
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustTarget = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustTarget;

        tomlInfo = craneLib.crateNameFromCargoToml { cargoToml = ./Cargo.toml; };
        inherit (tomlInfo) pname version;
        src = ./.;

        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          cargoExtraArgs = "--all-features --all";
        };

        nix_checks_junit = craneLib.buildPackage {
          inherit cargoArtifacts src version;
          cargoExtraArgs = "--all-features --all";
        };

      in
      rec {
        checks = {
          inherit nix_checks_junit;

          nix_checks_junit-clippy = craneLib.cargoClippy {
            inherit cargoArtifacts src;
            cargoExtraArgs = "--all --all-features";
            cargoClippyExtraArgs = "-- --deny warnings";
          };

          nix_checks_junit-fmt = craneLib.cargoFmt {
            inherit src;
          };
        };

        devShells.default = devShells.nix_checks_junit;
        devShells.nix_checks_junit = pkgs.mkShell {
          buildInputs = [ ];

          nativeBuildInputs = [
            rustTarget
          ];
        };
      }
    );
}
