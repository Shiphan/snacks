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
              # libclang.lib
              # libGL
              # dbus
            ];
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            LD_LIBRARY_PATH = builtins.foldl' (a: b: "${a}:${b}/lib") "${pkgs.vulkan-loader}/lib" buildInputs; # TODO: wtf is this
          };
      packages.${system}.default = pkgs.callPackage ./default.nix { };
      apps.${system}.default = {
        type = "app";
        program = "${self.packages.${system}.default}/bin/snacks";
      };
      formatter.${system} = pkgs.nixfmt-rfc-style;
    };
}
