# The Tauri desktop app (ui/ + ui/src-tauri/), built the nixpkgs way: npm dependencies from
# ui/package-lock.json, cargo dependencies from ui/src-tauri/Cargo.lock, `cargo tauri build`
# through cargo-tauri.hook, and a GApps wrapper.
#
# Both lockfiles carry a checksum per package, so there is no hash here to update: a fixed
# npmDeps hash broke at every release, since the version bump alone changes it.
{
  lib,
  src,
  rustPlatform,
  importNpmLock,
  cargo-tauri,
  nodejs,
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
  npmDeps = importNpmLock { npmRoot = src + "/ui"; };

  nativeBuildInputs = [
    cargo-tauri.hook
    nodejs
    importNpmLock.npmConfigHook
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
