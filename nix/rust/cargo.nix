{ pkgs, crane, toolchains, advisory-db, src }:
let
  inherit (pkgs) lib linkFarm;
  inherit (builtins) attrNames length listToAttrs mapAttrs;
  inherit (lib) concatStringsSep findSingle importJSON
    nameValuePair optionals optionalAttrs optionalString remove;

  baseLib = crane.mkLib pkgs;

  src = with baseLib; cleanCargoSource (path ./../..);

  commonArgs = {
    inherit src;
    pname = "bgpfu";
    strictDeps = true;
    doCheck = false;
  };

  buildDeps = { toolchainName, packageName, featureSet } @ args:
    toolchains.${toolchainName}.craneLib.buildDepsOnly (buildArgs args);

  buildArgs =
    { toolchainName
    , packageName
    , featureSet ? { name = "default"; set = null; }
    , extraExtraArgs ? ""
    , withDependencies ? false
    }:
    let
      inherit (featureSet) name set;
      featureArgs =
        if name == "default" then ""
        else if name == "all" then "--all-features"
        else "--no-default-features"
          + optionalString (length set > 0) " -F ${concatStringsSep "," set}";
      cargoArtifacts = buildDeps {
        inherit toolchainName packageName featureSet;
      };
    in
    commonArgs
    // {
      pname = "${packageName}-${toolchainName}-feature-set-${name}";
      cargoExtraArgs = "-p ${packageName} ${featureArgs} ${extraExtraArgs}";
    }
    // optionalAttrs withDependencies { inherit cargoArtifacts; };

  featureSets = features:
    let
      powerSet = list: builtins.foldl'
        (sublists: elem: sublists ++ map (sublist: sublist ++ [ elem ]) sublists)
        [ [ ] ]
        list;
      setName = set:
        if set == [ ] then "__empty"
        else concatStringsSep "+" set;
      nonDefaultFeatures = remove "default" features;
    in
    [
      { name = "default"; set = null; }
      { name = "all"; set = null; }
    ] ++ optionals (length features > 0) (map
      (set: { name = setName set; inherit set; })
      (powerSet nonDefaultFeatures));

  checkGroup = name: entries:
    let group = linkFarm "${name}-checks" entries; in
    group.overrideAttrs (_: prev: { passthru.checks = prev.passthru.entries; });

  checks = mapAttrs
    (toolchainName: { toolchain, craneLib }:
      let
        metadata = importJSON (craneLib.cargoMetadata commonArgs);
        packages = map
          ({ name, features, ... }: {
            inherit name;
            featureSets = featureSets (attrNames features);
          })
          metadata.packages;
      in
      checkGroup toolchainName {
        audit = craneLib.cargoAudit (commonArgs // {
          inherit advisory-db;
        });
        deny = craneLib.cargoDeny commonArgs;
        fmt = craneLib.cargoFmt commonArgs;
        taplo-fmt = craneLib.taploFmt (commonArgs // {
          taploExtraArgs = "--diff";
        });
        docs = checkGroup "docs" (map
          ({ name, ... }: {
            inherit name;
            path = craneLib.cargoDoc (buildArgs {
              inherit toolchainName;
              featureSet = { name = "all"; set = null; };
              packageName = name;
              withDependencies = true;
            } // {
              RUSTDOCFLAGS = concatStringsSep " " ([
                "-D warnings"
              ] ++ optionals (toolchainName == "nightly") [
                "--cfg docsrs "
              ]);
            });
          })
          packages);
        clippy = checkGroup "clippy" (map
          ({ name, featureSets }: {
            inherit name;
            path = checkGroup name (map
              (featureSet: {
                inherit (featureSet) name;
                path = craneLib.cargoClippy (buildArgs {
                  inherit toolchainName featureSet;
                  packageName = name;
                  withDependencies = true;
                } // {
                  cargoClippyExtraArgs = "--all-targets -- --deny warnings";
                });
              })
              featureSets);
          })
          packages);
        llvm-cov = checkGroup "llvm-cov" (map
          ({ name, featureSets }: {
            inherit name;
            path = checkGroup name (map
              (featureSet: {
                inherit (featureSet) name;
                path = checkGroup featureSet.name (lib.mapAttrsToList
                  (suiteName: cargoLlvmCovExtraArgs: {
                    name = suiteName;
                    path = craneLib.cargoLlvmCov (buildArgs {
                      inherit toolchainName featureSet;
                      packageName = name;
                      withDependencies = true;
                    } // {
                      inherit cargoLlvmCovExtraArgs;
                    });
                  })
                  {
                    all = "--lcov --output-path $out";
                  });
              })
              featureSets);
          })
          packages);
      })
    toolchains;

  devShells = mapAttrs
    (toolchainName: { craneLib, ... }:
    let
      checksForToolchain =
        let
          pred = value: value ? checks;
          name = path: lib.concatStringsSep "_" path;
          item = path: value: lib.nameValuePair (name path) value;
          mapRecursive = path: value:
            if lib.isAttrs value && pred value
            then recurse path value
            else [ (item path value) ];
          recurse = path: set: lib.concatMap
            (name: mapRecursive (path ++ [ name ]) set.checks.${name})
            (lib.attrNames set.checks);
        in lib.listToAttrs (recurse [ ] checks.${toolchainName});
      # checksForToolchain = checks.${toolchainName}.checks;
      # basicChecks = checks: lib.filterAtttrs (n: v: !(v ? checks)) checks;
      # clippyChecks = lib.mapAttrs' (n: v: nameValuePair "clippy-${n}" v.checks.default) checksForToolchain.clippy.checks;
      # llvmChecks = lib.mapAttrs' (n: v: nameValuePair "llvm-cov-${n}" v.checks.default) checksForToolchain.llvm-cov.checks;
    in
      craneLib.devShell {
        checks = checksForToolchain;
        # checks = clippyChecks // {
        #   inherit (checksForToolchain) audit deny fmt taplo-fmt llvm-cov;
        # };
      }
    )
    toolchains;


  buildBinWith = { platforms, toolchainName }:
    { pname
    , bin ? pname
    , defaultPlatform ? platforms.native
    , extraPlatforms ? [ ]
    }:
    let
      inherit (toolchains.${toolchainName}) craneLib;
      meta =
        let
          metadata = importJSON (craneLib.cargoMetadata commonArgs);
          packageMetadata = findSingle (p: p.name == pname)
            (throw "package ${pname} not found")
            (throw "duplicate metadata for package ${pname}")
            metadata.packages;
        in
        { inherit (packageMetadata) description; mainProgram = bin; };
      baseArgs = buildArgs
        {
          inherit toolchainName;
          packageName = pname;
          extraExtraArgs = "--bin ${bin}";
          withDependencies = true;
        } // { inherit meta pname; };
      passthru.platforms = listToAttrs (map
        ({ platformName, mkPackage }:
          nameValuePair platformName (mkPackage craneLib.buildPackage baseArgs))
        extraPlatforms);
    in
    defaultPlatform.mkPackage craneLib.buildPackage (baseArgs // { inherit passthru; });
in
{
  inherit buildBinWith checks devShells;
}
