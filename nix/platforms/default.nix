{ pkgs, jetez-src }:
{
  native = {
    platformName = "native";
    mkPackage = builder: args: builder args;
  };
  x86_64-junos-freebsd = import ./junos { inherit pkgs jetez-src; };
}
