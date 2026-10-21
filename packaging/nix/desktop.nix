# The Tauri desktop app (ui/ + ui/src-tauri/), built the nixpkgs way: npm dependencies from
# ui/package-lock.json, cargo dependencies from ui/src-tauri/Cargo.lock, `cargo tauri build`
# through cargo-tauri.hook, and a GApps wrapper.
#
# After changing ui/package-lock.json, update npmDeps.hash:
#   nix run nixpkgs#prefetch-npm-deps -- ui/package-lock.json
{
  lib,
  src,
  rustPlatform,
  fetchNpmDeps,
  cargo-tauri,
  nodejs,
  npmHooks,
  pkg-config,
  wrapGAppsHook3,
  webkitgtk_4_1,
  glib-networking,
  openssl,
  dbus,
  libayatana-appindicator,
}:

let
  version = (lib.importTOML (src + "/ui/src-tauri/Cargo.toml")).package.version;
in
rustPlatform.buildRustPackage {
  pname = "svrctlrs-desktop";
  inherit version src;

  cargoRoot = "ui/src-tauri";
  buildAndTestSubdir = "ui/src-tauri";
  cargoLock.lockFile = src + "/ui/src-tauri/Cargo.lock";

  npmRoot = "ui";
  npmDeps = fetchNpmDeps {
    name = "svrctlrs-ui-npm-deps-${version}";
    src = src + "/ui";
    hash = "sha256-5thGUU2tmEIrXve2p0M9YMmaNvbdwlqpwf0T2k55pzY=";
  };

  nativeBuildInputs = [
    cargo-tauri.hook
    nodejs
    npmHooks.npmConfigHook
    pkg-config
    wrapGAppsHook3
  ];

  buildInputs = [
    webkitgtk_4_1
    # TLS for the web view's requests to the server
    glib-networking
    openssl
    # The keyring (Secret Service over D-Bus)
    dbus
    # The tray; the bundler looks for it at build time too
    libayatana-appindicator
  ];

  # The tray library is loaded at run time (the app runs without it, but then has no tray)
  preFixup = ''
    gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ libayatana-appindicator ]})
  '';

  # The tests live in the server workspace; this crate has none
  doCheck = false;

  meta = {
    description = "SvrCtlRS desktop app: SSH terminals on your servers, signed in through your browser";
    homepage = "https://git.js-node.cc/jsprague/svrctlrs";
    license = lib.licenses.mit;
    mainProgram = "svrctlrs-desktop";
    platforms = lib.platforms.linux;
  };
}
