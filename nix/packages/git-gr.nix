{
  system,
  lib,
  stdenv,
  buildPackages,
  libiconv,
  darwin,
  inputs,
  rustPlatform,
  rust-analyzer,
  cargo-release,
  installShellFiles,
  pkg-config,
  openssl,
}: let
  inherit (inputs) crane advisory-db;
  craneLib = crane.lib.${system};
  src = lib.cleanSourceWith {
    src = craneLib.path ../../.;
    # Keep test data.
    filter = path: type:
      lib.hasInfix "/data" path
      || (craneLib.filterCargoSources path type);
  };

  commonArgs' = {
    inherit src;

    nativeBuildInputs =
      lib.optionals stdenv.isLinux [
        pkg-config
        openssl
      ]
      ++ lib.optionals stdenv.isDarwin [
        (libiconv.override {
          enableStatic = true;
          enableShared = false;
        })
        darwin.apple_sdk.frameworks.CoreServices
        darwin.apple_sdk.frameworks.SystemConfiguration
      ];
  };

  # Build *just* the cargo dependencies, so we can reuse
  # all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs';

  commonArgs =
    commonArgs'
    // {
      inherit cargoArtifacts;
    };

  checks = {
    git-gr-nextest = craneLib.cargoNextest (commonArgs
      // {
        NEXTEST_HIDE_PROGRESS_BAR = "true";
      });
    git-gr-doctest = craneLib.cargoTest (commonArgs
      // {
        cargoTestArgs = "--doc";
      });
    git-gr-clippy = craneLib.cargoClippy (commonArgs
      // {
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });
    git-gr-rustdoc = craneLib.cargoDoc (commonArgs
      // {
        cargoDocExtraArgs = "--document-private-items";
        RUSTDOCFLAGS = "-D warnings";
      });
    git-gr-fmt = craneLib.cargoFmt commonArgs;
    git-gr-audit = craneLib.cargoAudit (commonArgs
      // {
        inherit advisory-db;
      });
  };

  devShell = craneLib.devShell {
    inherit checks;

    # Make rust-analyzer work
    RUST_SRC_PATH = rustPlatform.rustLibSrc;

    # Extra development tools (cargo and rustc are included by default).
    packages = [
      rust-analyzer
      cargo-release
    ];
  };

  can-run-git-gr = stdenv.hostPlatform.emulatorAvailable buildPackages;
  git-gr = "${stdenv.hostPlatform.emulator buildPackages} $out/bin/git-gr";

  git-gr-man = craneLib.buildPackage (
    commonArgs
    // {
      cargoExtraArgs = "${commonArgs.cargoExtraArgs or ""} --features clap_mangen";

      nativeBuildInputs = commonArgs.nativeBuildInputs ++ [installShellFiles];

      postInstall =
        (commonArgs.postInstall or "")
        + lib.optionalString can-run-git-gr ''
          manpages=$(mktemp -d)
          ${git-gr} manpages "$manpages"
          for manpage in "$manpages"/*; do
            installManPage "$manpage"
          done

          installShellCompletion --cmd git-gr \
            --bash <(${git-gr} completions bash) \
            --fish <(${git-gr} completions fish) \
            --zsh <(${git-gr} completions zsh)

          rm -rf "$out/bin"
        '';
    }
  );
in
  # Build the actual crate itself, reusing the dependency
  # artifacts from above.
  craneLib.buildPackage (commonArgs
    // {
      # Don't run tests; we'll do that in a separate derivation.
      doCheck = false;

      postInstall =
        (commonArgs.postInstall or "")
        + ''
          cp -r ${git-gr-man}/share $out/share
          # What:
          chmod -R +w $out/share
        '';

      passthru = {
        inherit
          checks
          devShell
          commonArgs
          craneLib
          ;
      };
    })
