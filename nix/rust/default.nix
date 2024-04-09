{ pkgs, crane, fenix, platforms, nightly-manifest, stable-manifest, msrv-manifest, advisory-db }:
let
  inherit (pkgs) system lib;
  inherit (builtins) removeAttrs readFile listToAttrs map;
  inherit (lib) mapAttrsToList nameValuePair;

  toolchainManifests = {
    nightly = nightly-manifest;
    stable = stable-manifest;
    msrv = msrv-manifest;
  };

  toolchains = import ./toolchains.nix {
    inherit pkgs fenix platforms toolchainManifests;
  };

  cargo = import ./cargo.nix {
    inherit pkgs crane toolchains advisory-db;
    src = ./../..;
  };

  buildPackage = cargo.buildBinWith rec {
    inherit platforms;
    toolchainName = "stable";
  };
in
{
  inherit (cargo) checks;
  inherit buildPackage;
}
