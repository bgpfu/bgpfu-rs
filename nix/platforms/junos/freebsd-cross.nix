{ pkgs }:
with pkgs;
let
  freebsd-arch = "amd64";
  freebsd-major = 12;
  freebsd-minor = 4;

  target-arch = "x86_64";
  rust-target = "${target-arch}-unknown-freebsd";
  gnu-target = "${rust-target}${toString freebsd-major}";

  binutils = stdenv.mkDerivation
    rec {
      pname = "binutils-${gnu-target}";
      version = "2.32";
      src = fetchzip {
        url = "https://ftp.gnu.org/gnu/binutils/binutils-${version}.tar.gz";
        hash = "sha256-LUvvkE9/7fSrSFDBOqghKSQbLjWhKGXLUacpySHMwdY=";
      };
      enableParallelBuilding = true;
      configureFlags = [ "--target=${gnu-target}" ];
    };

  gcc =
    let
      freebsd-base =
        let
          version = "${toString freebsd-major}.${toString freebsd-minor}";
        in
        fetchzip {
          url = "https://ftp.freebsd.org/pub/FreeBSD/releases/${freebsd-arch}/${version}-RELEASE/base.txz";
          hash = "sha256-5UIyd6oZjBzcnC2E4MFftocorQfnIpbwAgZt0dhIDXE=";
          stripRoot = false;
        };
      fetch-gnu-src = { name, version, hash, compression ? "bz2" }: fetchzip {
        inherit hash;
        url = "https://gcc.gnu.org/pub/gcc/infrastructure/${name}-${version}.tar.${compression}";
      };
      mpfr-src = fetch-gnu-src {
        name = "mpfr";
        version = "2.4.2";
        hash = "sha256-LwiN1dYyIKLKLDWj4O1qzkTgh9iYLY8VTxpTPLtt5Bo=";
      };
      gmp-src = fetch-gnu-src {
        name = "gmp";
        version = "4.3.2";
        hash = "sha256-JJAmw32NfAl0Lq7AbK6EPCwqEWVBYHqvcg9gwuurbaQ=";
      };
      mpc-src = fetch-gnu-src {
        name = "mpc";
        version = "0.8.1";
        hash = "sha256-RElyn5c1mu18wiPiDC3s2QDss/sTCBM0On492Jk6K6k=";
        compression = "gz";
      };
    in
    stdenv.mkDerivation
      rec {
        pname = "gcc-${gnu-target}";
        version = "6.4.0";
        src = fetchzip {
          url = "https://ftp.gnu.org/gnu/gcc/gcc-${version}/gcc-${version}.tar.gz";
          hash = "sha256-TkyEvTY36r84a6rQDgvNRdy3W2uIYJ0e+KWquPc9GEs=";
        };
        nativeBuildInputs = [ binutils ];
        enableParallelBuilding = true;
        hardeningDisable = [ "format" "pie" ];
        sourceRoot = ".";
        postUnpack = /* bash */ ''
          ln -sf ${mpfr-src} source/mpfr
          ln -sf ${gmp-src} source/gmp
          ln -sf ${mpc-src} source/mpc
          mkdir build && cd build
        '';
        configureScript = "../source/configure";
        configureFlags = [
          "--disable-libada"
          "--disable-libcilkrt"
          "--disable-libcilkrts"
          "--disable-libgomp"
          "--disable-libquadmath"
          "--disable-libquadmath-support"
          "--disable-libsanitizer"
          "--disable-libssp"
          "--disable-libvtv"
          "--disable-lto"
          "--disable-nls"
          "--enable-languages=c,c++"
          "--target=${gnu-target}"
          "--with-sysroot=${freebsd-base}"
        ];
        passthru.linker = "${gnu-target}-gcc";
      };
in
{
      depsBuildBuild = [ binutils gcc ];
      CARGO_BUILD_TARGET = rust-target;
      CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_LINKER = gcc.linker;
}
