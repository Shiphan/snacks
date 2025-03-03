{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:

rustPlatform.buildRustPackage {
  pname = "snacks";
  # TODO: write this

  version = "unstable-2025-02-24";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.gitTracked ./.;
  };

  useFetchCargoVendor = true;
  cargoHash = "";

  meta = {
    # description = "";
    # homepage = "https://github.com/Shiphan/snacks";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ shiphan ];
    mainProgram = "snacks";
  };
}
