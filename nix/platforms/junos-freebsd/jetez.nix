{ jetez-src, pkgs }:
let
  jetez = pkgs.callPackage
    ({ src, lib, python3, openssl, cdrtools, ... }:
      python3.pkgs.buildPythonApplication {
        pname = "jetez";
        version = "v1.0.7";
        inherit src;
        buildInputs = [
          openssl
        ];
        propagatedBuildInputs = with python3.pkgs; [
          pyyaml
          lxml
        ];
        makeWrapperArgs = [
          "--prefix PATH : ${lib.makeBinPath [ openssl cdrtools ] }"
        ];
      })
    { src = jetez-src; };
  writeManifest = pkg:
    pkgs.writeText "${pkg.name}-jet-manifest" /* yaml */ ''
      basename: ${pkg.pname}
      comment: ${pkg.meta.description}
      copyright: "Copyright 2023, Workonline Communications"
      arch: "x86"
      abi: "64"
      files:
        - source: ${pkg.out}/bin/${pkg.meta.mainProgram}
          destination: /var/db/scripts/jet/${pkg.meta.mainProgram}
    '';
in
{
  mkJetPackage = { pkg, meta, passthru }:
    pkgs.runCommand
      "${pkg.name}-jet-package"
      { inherit meta passthru; }
      /* bash */ ''
      mkdir -p "$out"
      cd "$out"
      ${jetez}/bin/jetez \
        --source '.' \
        --version ${pkg.version} \
        --jet ${writeManifest pkg} \
        --cert "/certs/cert.pem" \
        --key "/certs/key.pem" \
        --build "../build" \
        --debug
    '';
}
