{ pkgs, crane, fenix, platforms, nightly-manifest, stable-manifest, msrv-manifest, advisory-db }:
let
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

  buildPackage = cargo.buildBinWith {
    inherit platforms;
    toolchainName = "stable";
  };
in
{
  inherit (cargo) checks devShells;
  inherit buildPackage;
}
