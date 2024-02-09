{
  description = "Packages and tooling for bgpfu";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    jetez-src = {
      url = "github:juniper/jetez/v1.0.7";
      flake = false;
    };
  };

  outputs = { self, ... } @ inputs:
    inputs.flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
          };
          platforms = import ./nix/platforms {
            inherit pkgs;
            inherit (inputs) jetez-src;
          };
          rust = import ./nix/rust.nix {
            inherit pkgs platforms;
            inherit (inputs) crane fenix;
          };
        in
        {
          packages = with platforms; rec {
            cli = rust.buildPackage {
              pname = "bgpfu-cli";
              bin = "bgpfu";
            };
            junos-agent = rust.buildPackage {
              pname = "bgpfu-junos-agent";
              defaultPlatform = x86_64-junos-freebsd;
              extraPlatforms = [ native ];
            };
            default = cli;
          };
        });
}
