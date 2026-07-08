{ jetez-src, pkgs }:
let
  jetez = pkgs.callPackage
    ({ src, lib, python3Packages, openssl, cdrtools, ... }:
      python3Packages.buildPythonApplication {
        pname = "jetez";
        version = "1.0.7";
        pyproject = true;
        inherit src;
        build-system = with python3Packages; [ setuptools ];
        dependencies = with python3Packages; [
          pyyaml
          lxml
        ];
        buildInputs = [ openssl ];
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
