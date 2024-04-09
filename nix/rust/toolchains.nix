{ pkgs, fenix, platforms, toolchainManifests }:
let
  inherit (pkgs) system lib;
  inherit (builtins) mapAttrs map;
  inherit (lib) filterAttrs mapAttrsToList;

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
    ] ++ crossComponents manifest;

in
mapAttrs
  (_: manifest: fenixPkgs.combine (components manifest))
  toolchainManifests
