{
  # The SvrCtlRS desktop app as a Nix package, for NixOS (Wayland included). The AppImage forces
  # X11 (Tauri's GTK hook) and cannot find libraries on NixOS; this build uses nixpkgs' own
  # WebKitGTK and is wrapped with what GTK, TLS and the tray need at run time.
  #
  #   nix build .#desktop          ./result/bin/svrctlrs-desktop
  #   nix run git+https://git.js-node.cc/jsprague/svrctlrs#desktop
  #
  # In a NixOS configuration: add this flake as an input and put
  # `inputs.svrctlrs.packages.${pkgs.system}.desktop` in environment.systemPackages.
  description = "SvrCtlRS — a self-hosted SSH terminal manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forSystems (pkgs: rec {
        desktop = pkgs.callPackage ./packaging/nix/desktop.nix { src = self; };
        default = desktop;
      });
    };
}
