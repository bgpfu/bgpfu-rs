{ crane, fenix, pkgs, platforms }:
let
  inherit (pkgs) system lib;
  inherit (builtins) removeAttrs fromTOML readFile listToAttrs map;
  inherit (lib) mapAttrsToList nameValuePair;

  toolchain = with fenix.packages.${system}; combine ([
    stable.rustc
    stable.cargo
  ] ++ mapAttrsToList
    (_: { rustTarget ? null, ... }:
      targets.${rustTarget}.stable.rust-std)
    (removeAttrs platforms [ "native" ]));

  craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

  src = craneLib.cleanCargoSource (craneLib.path ./..);

  manifest = fromTOML (readFile (src + "/Cargo.toml"));

in
{
  buildPackage =
    { pname
    , bin ? pname
    , defaultPlatform ? platforms.native
    , extraPlatforms ? [ ]
    }:
    let
      baseArgs = {
        inherit pname src;
        strictDeps = true;
        cargoExtraArgs = "--bin ${bin}";
        meta = {
          inherit (manifest.workspace.package) description;
          mainProgram = bin;
        };
      };
      passthru.platforms = listToAttrs (map
        ({ platformName, mkPackage }:
          nameValuePair platformName (mkPackage craneLib.buildPackage baseArgs))
        extraPlatforms);
    in
    defaultPlatform.mkPackage craneLib.buildPackage (baseArgs // { inherit passthru; });
}
