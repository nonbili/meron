{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    bun
    coreutils
    gcc
    go
    rustup
    sqlite
  ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
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
    echo "Install the Wails CLI with: go install github.com/wailsapp/wails/v2/cmd/wails@latest"
  '';
}
