{ pkgs }:
let
  platformName = "junos-evo";
  rustTarget = "x86_64-unknown-linux-gnu";
  jet-evo = import ./jet-evo.nix { inherit pkgs; };
  cc =
    let
      inherit (pkgs) wrapCCWith wrapBintoolsWith gcc-unwrapped bintools-unwrapped;
      glibc = (import
        (builtins.fetchTarball {
          url = "https://github.com/NixOS/nixpkgs/archive/136a26be29a9daa04e5f15ee7694e9e92e5a028c.tar.gz";
          sha256 = "sha256:0g0lnngv9v7df5m1f7265y90106c52saslhz5pkylakgs2q33860";
        })
        { inherit (pkgs) system; }).glibc;
    in
    wrapCCWith {
      cc = gcc-unwrapped;
      bintools = wrapBintoolsWith {
        bintools = bintools-unwrapped;
        libc = glibc;
      };
    };
in
{
  inherit platformName rustTarget;
  mkPackage = builder: { pname, passthru ? { }, meta, bin, ... } @ args:
    let
      finalArgs = args // {
        pname = "${pname}-${platformName}";
        doCheck = false;
        depsBuildBuild = [ cc ];
        CARGO_BUILD_TARGET = rustTarget;
        postFixup = ''
          echo "setting dynamic linker for junos evo platform"
          patchelf --set-interpreter "/lib64/ld-linux-x86-64.so.2" $out/bin/${bin}
        '';
      };
    in
    jet-evo.mkJetPackage {
      pkg = builder finalArgs;
      inherit meta passthru;
    };
}
