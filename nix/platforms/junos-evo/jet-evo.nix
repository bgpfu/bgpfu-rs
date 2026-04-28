{ pkgs }: {
  mkJetPackage = { pkg, meta, passthru ? { } }:
    let
      inherit (pkg.meta) mainProgram;
      binDir = "/usr/sbin";
      binPath = "${binDir}/${mainProgram}";
      # TODO:
      args = "--irrd-host irr.wolcomm.net -l /var/log/bgpfu-junos-agent -f 300";
      sysManConfig =
        (pkgs.formats.yaml { }).generate "${mainProgram}.yaml" {
          binpath = binPath;
          exec-start = "${binPath} ${args}";
          working-dir = binDir;
          id = mainProgram;
          network = "external";
          on-exit.restart = true;
          resource = {
            instances = {
              all_nodes = false;
              max_num_of_instances = 1;
            };
            node-attributes = [ "re" ];
            max-memory = "2G";
            startup = true;
          };
        };
    in
    pkgs.runCommand
      "${pkg.name}-jet-package"
      {
        inherit meta;
        passthru = passthru // { inherit pkg; };
        nativeBuildInputs = with pkgs; [
          ima-evm-utils
          python3
          squashfsTools
        ];
        OECORE_DISTRO_VERSION = "3.0.2";
      }
      /* bash */ ''
      set -xeuo pipefail

      # set up jet package root
      pkg_root="pkg_root"
      mkdir -p $pkg_root/usr/{sbin,conf}
      cp "${pkg.out}/bin/${mainProgram}" "$pkg_root/usr/sbin/"
      cp "${sysManConfig}" "$pkg_root/usr/conf/${mainProgram}.yaml"

      # build jet package
      pkg_build="pkg_build"
      python ${./Jet-evo} \
        --name "${mainProgram}" \
        --root "$pkg_root" \
        --version "${pkg.version}" \
        --directory "$pkg_build" \
        --key "/certs/key.pem" \
        --tar

      # copy built package to out-path
      mkdir -p $out
      mv "$pkg_build/${mainProgram}.${pkg.version}.tgz" \
         "$out/${pkg.pname}-x86-64-${pkg.version}.tgz"
    '';
}
