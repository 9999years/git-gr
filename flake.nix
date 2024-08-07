{
  description = "A Gerrit CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    systems.url = "github:nix-systems/default";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  nixConfig = {
    extra-substituters = ["https://cache.garnix.io"];
    extra-trusted-substituters = ["https://cache.garnix.io"];
    extra-trusted-public-keys = ["cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="];
  };

  outputs = inputs @ {
    self,
    nixpkgs,
    systems,
    crane,
    rust-overlay,
    advisory-db,
  }: let
    eachSystem = nixpkgs.lib.genAttrs (import systems);
  in {
    _pkgs = eachSystem (
      localSystem:
        import nixpkgs {
          inherit localSystem;
          overlays = [
            rust-overlay.overlays.default

            (
              final: prev: {
                rustToolchain = final.pkgsBuildHost.rust-bin.stable.latest.default.override {
                  targets =
                    final.lib.optionals final.stdenv.targetPlatform.isDarwin [
                      "x86_64-apple-darwin"
                      "aarch64-apple-darwin"
                    ]
                    ++ final.lib.optionals final.stdenv.targetPlatform.isLinux [
                      "x86_64-unknown-linux-musl"
                      "aarch64-unknown-linux-musl"
                    ];
                  extensions = ["llvm-tools-preview"];
                };

                craneLib = (crane.mkLib final).overrideToolchain final.rustToolchain;
              }
            )
          ];
        }
    );

    packages = eachSystem (localSystem: let
      pkgs = self._pkgs.${localSystem};
      inherit (pkgs) lib;
      packages = pkgs.callPackage ./nix/makePackages.nix {inherit inputs;};
    in
      (lib.filterAttrs (name: value: lib.isDerivation value) packages)
      // {
        default = packages.git-gr;

        # This lets us use `nix run .#cargo` to run Cargo commands without
        # loading the entire `nix develop` shell (which includes
        # `rust-analyzer`).
        #
        # Used in `.github/workflows/release.yaml`.
        cargo = pkgs.cargo;
      });

    checks = eachSystem (system: self.packages.${system}.default.checks);

    devShells = eachSystem (system: {
      default = self.packages.${system}.default.devShell;
    });
  };
}
