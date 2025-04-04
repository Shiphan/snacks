{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default =
        pkgs.mkShell.override
          {
            stdenv = pkgs.clangStdenv;
          }
          rec {
            buildInputs = with pkgs; [
              pkg-config
              libxkbcommon.dev
              pipewire.dev
              wayland
              vulkan-loader
              # libclang.lib
              # libGL
              # dbus
            ];
            LIBCLANG_PATH = nixpkgs.lib.makeLibraryPath [ pkgs.libclang.lib ];
            LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath buildInputs;
          };
      packages.${system}.default = pkgs.callPackage ./default.nix { };
      apps.${system}.default = {
        type = "app";
        program = "${self.packages.${system}.default}/bin/snacks";
      };
      formatter.${system} = pkgs.nixfmt-rfc-style;
    };
}
