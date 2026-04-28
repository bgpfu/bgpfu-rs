{ pkgs, jetez-src }:
{
  native = {
    platformName = "native";
    mkPackage = builder: args: builder args;
  };
  x86_64-junos-freebsd = import ./junos-freebsd { inherit pkgs jetez-src; };
  x86_64-junos-evo = import ./junos-evo { inherit pkgs; };
}
