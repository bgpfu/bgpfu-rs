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
        else "--no-default-features"
          + optionalString (length set > 0) " -F ${concatStringsSep "," set}";
      buildDeps = { toolchainName, ... } @ args:
        let
          toolchain = toolchains.${toolchainName};
          craneLib = mkLib toolchain;
        in
        craneLib.buildDepsOnly (buildArgs args);
    in
    commonArgs // {
      pname = "${packageName}-${toolchainName}-feature-set-${name}";
      cargoExtraArgs = "-p ${packageName} ${featureArgs} ${extraExtraArgs}";
    } // optionalAttrs withDependencies {
      cargoArtifacts = buildDeps {
        inherit toolchainName packageName featureSet;
      };
    };


  mkLib = toolchain:
    let craneLib = baseLib.overrideToolchain toolchain; in
    craneLib // {
      cargoMetadata = { ... } @ args: craneLib.mkCargoDerivation (args // {
        cargoArtifacts = null;
        pnameSuffix = "-metadata";
        buildPhaseCargoCommand = "cargo metadata --no-deps --format-version 1 >$out";
        doInstallCargoArtifacts = false;
        installPhaseCommand = "";
      });
    };

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
    [{ name = "default"; set = null; }]
    ++ optionals (length features > 0) (map
      (set: { name = setName set; inherit set; })
      (powerSet nonDefaultFeatures));

  checkGroup = name: entries:
    let group = linkFarm "${name}-checks" entries; in
    group.overrideAttrs (_: prev: { passthru.checks = prev.passthru.entries; });

  checks = mapAttrs
    (toolchainName: toolchain:
      let
        craneLib = mkLib toolchain;
        metadata = importJSON (craneLib.cargoMetadata commonArgs);
        packages = map
          ({ name, features, ... }: {
            inherit name;
            featureSets = featureSets (attrNames features);
          })
          metadata.packages;
        clippy = { name, featureSets, ... }:
          checkGroup name (map
            (featureSet: {
              inherit (featureSet) name;
              path = craneLib.cargoClippy (buildArgs
                {
                  inherit toolchainName featureSet;
                  packageName = name;
                  withDependencies = true;
                } // {
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              });
            })
            featureSets);
      in
      checkGroup toolchainName {
        audit = craneLib.cargoAudit (commonArgs // {
          inherit advisory-db;
        });
        deny = craneLib.cargoDeny commonArgs;
        fmt = craneLib.cargoFmt commonArgs;
        clippy = checkGroup "clippy" (map
          (package: {
            inherit (package) name;
            path = clippy package;
          })
          packages);
      })
    toolchains;

  buildBinWith = { platforms, toolchainName }:
    { pname
    , bin ? pname
    , defaultPlatform ? platforms.native
    , extraPlatforms ? [ ]
    }:
    let
      toolchain = toolchains.${toolchainName};
      craneLib = mkLib toolchain;
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
  inherit buildBinWith checks;
}
