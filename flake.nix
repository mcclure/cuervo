{
  description = "Cuervo terminal based browser embedded TUI app, using Servo browser engine embedded";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, nixpkgs-unstable, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system;
          overlays = overlays;
        };
        unstable = import nixpkgs-unstable {
          inherit system;
          overlays = overlays;
        };
        rustPlatform = pkgs.makeRustPlatform {
          cargo = pkgs.cargo;
          rustc = pkgs.rustc;
        };
        llvmPackage = pkgs.llvmPackages_21;
      in rec {
        checks = {
          clippy = pkgs.writeShellApplication {
            name = "clippy-check";
            runtimeInputs = [ pkgs.rustc pkgs.cargo ];
            text = ''
              cargo clippy --all-targets --all-features -- -D warnings
            '';
          };

          fmt = pkgs.writeShellApplication {
            name = "fmt-check";
            runtimeInputs = [ pkgs.rustfmt ];
            text = ''
              cargo fmt -- --check
            '';
          };

          test = pkgs.writeShellApplication {
            name = "test-check";
            runtimeInputs = [ pkgs.rustc pkgs.cargo ];
            text = ''
              cargo test
            '';
          };

          # NOTE: cuervo doesnt have a one-off output on process call thus this is commented out for now:
          # output = pkgs.nixosTest {
          #   name = "rust-output-test";
          #   nodes.machine = { config, pkgs, ... }: {
          #     environment.systemPackages = [ self.packages.${system}.default ];
          #     system.stateVersion = pkgs.lib.versions.majorMinor pkgs.lib.version;
          #   };
          #
          #   testScript = ''
          #     machine.wait_for_unit("default.target")
          #     # machine.succeed("cuervo --help | grep -o \"Cuervo\")
          #
          #     # machine.succeed("systemd-run --unit=cuervo cuervo --help")
          #   '';
          # };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            pkgs.rustfmt
            pkgs.clippy

            self.packages.${system}.default.nativeBuildInputs
          ];

          buildInputs = [
            self.packages.${system}.default.buildInputs
          ];

          doCheck = false; # Disables automatically running tests for `$ nix develop` and direnv

          # Warnings due to llvmPackages.stdenv result in compile error on debug, improve the llvmPackages.stdenv by
          # `cargo build -r` worked but `cargo build` doesnt work
          shellHook = ''
            export CC="${llvmPackage.stdenv.cc}/bin/clang";
            export CXX="${llvmPackage.stdenv.cc}/bin/clang++";
            export LIBCLANG_PATH="${llvmPackage.libclang.lib}/lib";
            export BINDGEN_CFLAGS="$(< ${llvmPackage.stdenv.cc}/nix-support/libc-crt1-cflags) \
              $(< ${llvmPackage.stdenv.cc}/nix-support/libc-cflags) \
              $(< ${llvmPackage.stdenv.cc}/nix-support/cc-cflags) \
              $(< ${llvmPackage.stdenv.cc}/nix-support/libcxx-cxxflags) \
              -idirafter ${llvmPackage.stdenv.cc.cc.lib}/lib/clang/${pkgs.lib.getVersion llvmPackage.stdenv.cc.cc}/include"
            export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.wayland}/lib";

            export ZDOTDIR=$(mktemp -d)
            cat > "$ZDOTDIR/.zshrc" << 'EOF'
              source ~/.zshrc # Source the original ~/.zshrc, required.

              function parse_git_branch {
                git branch --no-color 2> /dev/null | sed -e '/^[^*]/d' -e 's/* \(.*\)/\ ->\ \1/'
              }

              function display_jobs_count_if_needed {
                local job_count=$(jobs -s | wc -l | tr -d " ")

                if [ $job_count -gt 0 ]; then
                  echo "%B%F{yellow}%j| ";
                fi
              }

              # NOTE: Custom prompt with a snowflake: signals we are in `$ nix develop` shell
              PROMPT="%F{blue}$(date +%H:%M:%S) $(display_jobs_count_if_needed)%B%F{green}%n %F{blue}%~%F{cyan} ❄%F{yellow}$(parse_git_branch) %f%{$reset_color%}"
            EOF

            if [ -z "$DIRENV_IN_ENVRC" ]; then # This makes `$ nix develop` universally working with direnv without infinite loop
              exec ${pkgs.zsh}/bin/zsh -i
            fi
          '';
        };

        formatter = pkgs.nixpkgs-fmt;

        apps = {
          default = {
            type = "app";
            program = "${self.packages.${system}.default}/bin/cuervo";
          };
        };

        # TODO: Finish it up below:
        packages = rec {
          # TODO: Do this:
          default = rustPlatform.buildRustPackage {
            pname = "cuervo";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoVendorDir = null; # 👈 disable nix cargo vendor isolation, it was needed for servo vendoring before
            cargoLock.outputHashes = {
              "background_hang_monitor-0.0.1" = "sha256-z9JQ9rrlDWYyf+nRgjru3VIcK5MiMdnrq6LOFFGx5EY=";
              "dom-0.0.1" = "sha256-rOEvolq3NydznfeM9eq8n1Cl3E1bds1wyzhoerYIWJs=";
              "fontsan-0.5.2" = "sha256-4id66xxQ8iu0+OvJKH77WYPUE0eoVa9oUHmr6lRFPa8=";
              "mozjs-0.14.1" = "sha256-q+kGrz2A6HRgJNtnUsc9btTjgXHyFRM0mxG/6QUfZYI=";
              "naga-24.0.0" = "sha256-QxEAJP/N8GgwRrHHIvPV0u2OSelMAbhk2/u9bP8fQlg=";
              "peek-poke-0.3.0" = "sha256-WCZYX68vZrPhaAZwpx9/lUp3bVsLMwtmlJSW8wNb2ks=";
              "servo-media-0.1.0" = "sha256-KISNnnOjM6Fuxd6JkAlq2o4Wn5ChhN9UDYPPq2kCzvs=";
              "signpost-0.1.0" = "sha256-xRVXwW3Gynace9Yk5r1q7xA60yy6xhC5wLAyMJ6rPRs=";
              "surfman-0.9.8" = "sha256-bF5Oiw0ZRavovOaaFRegX55CdFFo/yuDn9lk+JEyCcA=";
              "webxr-0.0.1" = "sha256-Hc71BvOFCo41HZtTCp1Hj6YBAwxQJgIzSEMVMM9y6JE=";
            };
            nativeBuildInputs = [ 
              pkgs.pkg-config 
              llvmPackage.clang
              llvmPackage.libclang
            ];
            buildInputs = [ 
              pkgs.wayland
              pkgs.xorg.libX11.dev
              pkgs.fontconfig
              pkgs.libunwind
            ];
            doCheck = false; # NOTE: When there are tests, do this true
            # # NOTE: This still doesnt pass: include!("../style/counter_style/predefined.rs"); error on servo_atoms
            # preBuild = ''
            #   mkdir -p vendor/servo
            #   cp -r ${pkgs.fetchgit {
            #     url = "https://github.com/servo/servo";
            #     rev = "3a9476469ba4bbc0297b0d06c1ccdd4261f6f3ee";
            #     sha256 = "sha256-z9JQ9rrlDWYyf+nRgjru3VIcK5MiMdnrq6LOFFGx5EY=";
            #   }} vendor/servo
            # '';
            # # NOTE: try this:
            # preBuild = ''
            #   mkdir -p other
            #   if [ ! -d other/servo ]; then
            #     echo "Fetching Servo (with submodules)..."
            #     ${pkgs.git}/bin/git clone --recursive https://github.com/mcclure/servo other/servo
            #     (cd other/servo && ${pkgs.git}/bin/git checkout 3a9476469ba4bbc0297b0d06c1ccdd4261f6f3ee)
            #   fi
            # '';

            postInstall = ''
              echo "Installed cuervo to $out/bin"
            '';
            meta = {
              description = "An example Rust binary built with Nix flakes";
              license = pkgs.lib.licenses.mit;
            };
          };

          development = default;

          production = default;

          # dockerImage = pkgs.dockerTools.buildLayeredImage {
          #   name = "cuervo";
          #   tag = "dev";
          #   created = "now";
          #   contents = [
          #     pkgs.zsh
          #     pkgs.coreutils
          #     # pkgs.cacert
          #     self.packages.${system}.default
          #   ];
          #   config = {
          #     # Env = [
          #     #   "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          #     # ];
          #     Cmd = [ "/bin/cuervo" ];
          #   };
          # };
        };
      }
    );
}
