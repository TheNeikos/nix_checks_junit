{
  description = "The nix_checks_junit binary";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-26.05";
    crane = {
      url = "github:ipetkov/crane";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs =
    inputs@{
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      eachSupportedSystem = inputs.nixpkgs.lib.genAttrs supportedSystems;

      pkgsFor =
        system:
        import inputs.nixpkgs {
          inherit system;
          overlays = [ (import inputs.rust-overlay) ];
        };

      rustTargetFor = system: (pkgsFor system).rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      craneLibFor = system: (crane.mkLib (pkgsFor system)).overrideToolchain (rustTargetFor system);

      tomlInfoFor = system: (craneLibFor system).crateNameFromCargoToml { cargoToml = ./Cargo.toml; };

      cargoArtifactsFor =
        system:
        (craneLibFor system).buildDepsOnly {
          src = ./.;
          cargoExtraArgs = "--all-features --all";
        };

      nix_checks_junitFor =
        system:
        let
          cargoArtifacts = cargoArtifactsFor system;
          crane = craneLibFor system;
          tomlInfo = tomlInfoFor system;
          pkgs = pkgsFor system;
        in
        crane.buildPackage {
          inherit cargoArtifacts;
          inherit (tomlInfo) version;
          src = ./.;
          cargoExtraArgs = "--all-features --all";

          meta = {
            mainProgram = "nix_checks_junit";
            description = "A 'nix flake check' wrapper, that outputs junit compatible XML reports.";
            homepage = "https://github.com/TheNeikos/nix_checks_junit";
            license = pkgs.lib.licenses.agpl3Plus;
          };
        };

    in
    {
      checks = eachSupportedSystem (
        system:
        let
          cargoArtifacts = cargoArtifactsFor system;
          craneLib = craneLibFor system;
          nix_checks_junit = nix_checks_junitFor system;
        in
        {
          inherit nix_checks_junit;

          nix_checks_junit-clippy = craneLib.cargoClippy {
            inherit cargoArtifacts;
            src = ./.;
            cargoExtraArgs = "--all --all-features";
            cargoClippyExtraArgs = "-- --deny warnings";
          };

          nix_checks_junit-fmt = craneLib.cargoFmt {
            src = ./.;
          };
        }
      );

      packages = eachSupportedSystem (system: {
        nix_checks_junit = nix_checks_junitFor system;
        default = nix_checks_junitFor system;
      });

      devShells = eachSupportedSystem (system: {
        default = (pkgsFor system).mkShell {
          buildInputs = [ ];

          nativeBuildInputs = [
            (rustTargetFor system)
          ];
        };
      });
    };
}
