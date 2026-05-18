{ pkgs, crane, fenix, platforms, toolchainManifests }:
let
  inherit (pkgs) lib;
  inherit (pkgs.stdenv.hostPlatform) system;
  inherit (lib) filterAttrs map mapAttrs mapAttrsToList;

  fenixPkgs = fenix.packages.${system};

  crossComponents = manifest:
    let
      crossPlatforms = filterAttrs
        (_: { rustTarget ? null, ... }: rustTarget != null)
        platforms;
      crossTargets = mapAttrsToList
        (_: { rustTarget, ... }: rustTarget)
        crossPlatforms;
      crossToolchain = target:
        fenixPkgs.targets.${target}.fromManifestFile manifest;
    in
    map (target: (crossToolchain target).rust-std) crossTargets;

  components = manifest:
    with fenixPkgs.fromManifestFile manifest; [
      rustc
      cargo
      clippy
      rustfmt
      llvm-tools
      rust-analyzer
      rust-src
    ] ++ crossComponents manifest;

  mkCraneLib = toolchain:
    let
      baseLib = crane.mkLib pkgs;
      craneLib = baseLib.overrideToolchain toolchain;
    in
    craneLib // {
      cargoMetadata = { ... } @ args: craneLib.mkCargoDerivation (args // {
        cargoArtifacts = null;
        pnameSuffix = "-metadata";
        buildPhaseCargoCommand = "cargo metadata --no-deps --format-version 1 >$out";
        doInstallCargoArtifacts = false;
        installPhaseCommand = "";
      });
    };
in
mapAttrs
  (_: manifest: rec {
    toolchain = fenixPkgs.combine (components manifest);
    craneLib = mkCraneLib toolchain;
  })
  toolchainManifests
