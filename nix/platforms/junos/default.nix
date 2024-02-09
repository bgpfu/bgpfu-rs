{ pkgs, jetez-src }:
let
  platformName = "junos-freebsd";
  jetez = import ./jetez.nix { inherit jetez-src pkgs; };
  freebsdCrossArgs = import ./freebsd-cross.nix { inherit pkgs; };
in
{
  inherit platformName;
  rustTarget = freebsdCrossArgs.CARGO_BUILD_TARGET;
  mkPackage = builder: { pname, passthru, meta, ... } @ args:
    let
      finalArgs = args // freebsdCrossArgs // {
        pname = "${pname}-${platformName}";
        doCheck = false;
      };
    in
    jetez.mkJetPackage {
      pkg = builder finalArgs;
      inherit meta passthru;
    };
}
