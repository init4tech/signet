{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Native dependencies for reth / mdbx / rocksdb
    pkg-config
    openssl.dev
    jemalloc
    llvmPackages.libclang.lib
    clang
  ];

  # jemalloc: use system lib instead of building from source
  JEMALLOC_OVERRIDE = "${pkgs.jemalloc}/lib/libjemalloc.so";

  # bindgen needs libclang
  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  # bindgen needs C standard headers (stdarg.h etc.)
  BINDGEN_EXTRA_CLANG_ARGS = "-I${pkgs.gcc.cc}/lib/gcc/${pkgs.stdenv.hostPlatform.config}/${pkgs.gcc.cc.version}/include";
}
