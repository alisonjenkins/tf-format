{
  description = "tf-format - Opinionated Terraform/OpenTofu HCL formatter";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        packages = {
          tf-format = pkgs.rustPlatform.buildRustPackage {
            pname = "tf-format";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };

          default = self.packages.${system}.tf-format;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [self.packages.${system}.tf-format];

          packages = with pkgs; [
            rust-analyzer
            clippy
            rustfmt
          ];
        };
      }
    );
}
