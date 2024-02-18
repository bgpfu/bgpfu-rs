{ pkgs, jetez-src }:
let
  platformName = "junos-freebsd";
  jetez = import ./jetez.nix { inherit jetez-src pkgs; };
  freebsdCross = import ./freebsd-cross.nix { inherit pkgs; };
in
{
  inherit platformName;
  inherit (freebsdCross) rustTarget;
  mkPackage = builder: { pname, passthru, meta, ... } @ args:
    let
      finalArgs = args // {
        pname = "${pname}-${platformName}";
        doCheck = false;
        depsBuildBuild = with freebsdCross; [
          binutils
          gcc
        ];
        CARGO_BUILD_TARGET = freebsdCross.rustTarget;
        CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_LINKER = freebsdCross.gcc.linker;
        RUSTFLAGS = ''--cfg target_platform="${platformName}"'';
      };
    in
    jetez.mkJetPackage {
      pkg = builder finalArgs;
      inherit meta passthru;
    };
}
