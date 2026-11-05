{ pkgs ? import <nixpkgs> {} }:

let
  # On macOS the Apple toolchain has to stay in charge: Go/cgo, Wails, codesign
  # and altool all break in confusing ways once nix's stdenv C toolchain takes
  # over, because it hijacks cc, DEVELOPER_DIR/SDKROOT and xcrun. mkShellNoCC
  # keeps nix out of the compiler's way — this shell is only here for the tools
  # below, above all rustup, which the universal Mac App Store build needs for
  # the second Rust target. Linux does want a compiler, for cgo and webkit.
  mkShell = if pkgs.stdenv.isDarwin then pkgs.mkShellNoCC else pkgs.mkShell;
in
mkShell {
  packages = with pkgs; [
    bun
    coreutils
    go
    rustup
    sqlite
  ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
    gcc
    gtk3
    pkg-config
    webkitgtk_4_1
  ];

  shellHook = ''
    ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
      export GIO_MODULE_DIR="${pkgs.glib-networking}/lib/gio/modules"
      export XDG_DATA_DIRS="${pkgs.gtk3}/share:${pkgs.gsettings-desktop-schemas}/share:''${XDG_DATA_DIRS:-}"
    ''}
    echo "Meron dev shell: bun, Go, Rust, and sqlite are available."
    echo "Install the Wails CLI with: go install -tags gtk3 github.com/wailsapp/wails/v3/cmd/wails3@v3.1.0"
  '';
}
