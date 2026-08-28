package main

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

// Install channels the desktop app can be running from. The kind decides both
// which asset the updater downloads and how it is applied.
const (
	channelDMG      = "dmg"      // macOS .app inside /Applications, shipped as a .dmg
	channelMAS      = "mas"      // macOS .app installed from the Mac App Store
	channelAppImage = "appimage" // single-file Linux AppImage
	channelTarball  = "tarball"  // raw Linux binary from the .tar.gz
	channelNSIS     = "nsis"     // Windows install from the NSIS installer
	channelPortable = "portable" // Windows loose meron.exe from the portable zip
	channelSnap     = "snap"
	channelFlatpak  = "flatpak"
	channelAppx     = "appx"
	channelUnknown  = "unknown"
)

// updateChannel describes how this copy of Meron was installed.
type updateChannel struct {
	// Kind is one of the channel* constants above.
	Kind string
	// Managed is true when a package manager owns updates, either because a
	// store was detected or MERON_DISABLE_SELF_UPDATE opted out explicitly.
	// The updater stays out of the way and the UI says so.
	Managed bool
	// Target is what an update replaces: the .app bundle root on macOS, the
	// AppImage file on Linux, the executable elsewhere. Empty when Managed.
	Target string
}

// SelfUpdatable reports whether this channel supports in-app updates.
func (c updateChannel) SelfUpdatable() bool {
	return !c.Managed && c.Target != "" && c.Kind != channelUnknown
}

// detectChannel inspects the environment and the running executable's path to
// work out which packaging this build came from.
func detectChannel() updateChannel {
	exe, err := os.Executable()
	if err != nil {
		return updateChannel{Kind: channelUnknown}
	}
	// Resolve symlinks the same way installDesktopEntry does, so a
	// /usr/local/bin/meron -> /opt/meron/meron shim points at the real file.
	if resolved, err := filepath.EvalSymlinks(exe); err == nil {
		exe = resolved
	}
	return detectChannelFrom(runtime.GOOS, exe, os.Getenv)
}

// detectChannelFrom is the testable core of detectChannel: everything it needs
// about the host is passed in.
func detectChannelFrom(goos, exe string, getenv func(string) string) (channel updateChannel) {
	// Package maintainers can opt out without patching the source. Keep the
	// detected kind and target for diagnostics, but report the installation as
	// externally managed so the UI hides self-update controls and network checks
	// are skipped.
	defer func() {
		if envFlagEnabled(getenv("MERON_DISABLE_SELF_UPDATE")) {
			channel.Managed = true
		}
	}()

	// Store-managed packagings first — each exports a telltale variable into
	// the app's environment, and inside those sandboxes the paths below would
	// otherwise look like an ordinary install.
	if getenv("SNAP") != "" || getenv("SNAP_NAME") != "" {
		return updateChannel{Kind: channelSnap, Managed: true}
	}
	if getenv("FLATPAK_ID") != "" {
		return updateChannel{Kind: channelFlatpak, Managed: true}
	}

	switch goos {
	case "windows":
		// MSIX/appx apps run from the read-only WindowsApps store directory.
		if strings.Contains(strings.ToLower(exe), `\windowsapps\`) {
			return updateChannel{Kind: channelAppx, Managed: true}
		}
		// The Wails NSIS template drops an uninstaller beside the exe; the
		// portable zip contains nothing but meron.exe.
		if fileExists(filepath.Join(filepath.Dir(exe), "uninstall.exe")) {
			return updateChannel{Kind: channelNSIS, Target: exe}
		}
		return updateChannel{Kind: channelPortable, Target: exe}

	case "darwin":
		bundle := appBundleRoot(exe)
		if bundle == "" {
			return updateChannel{Kind: channelUnknown}
		}
		// The App Store wraps every install with a receipt. Those bundles are
		// owned by the store — replacing one in place would break the signature
		// and App Review rejects apps that update themselves — so report it as
		// managed and let the UI say so.
		if fileExists(filepath.Join(bundle, "Contents", "_MASReceipt", "receipt")) {
			return updateChannel{Kind: channelMAS, Managed: true}
		}
		return updateChannel{Kind: channelDMG, Target: bundle}

	case "linux":
		// The AppImage runtime exports APPIMAGE with the path of the image
		// itself; exe points inside the temporary mount, which is read-only.
		if image := getenv("APPIMAGE"); image != "" {
			return updateChannel{Kind: channelAppImage, Target: image}
		}
		return updateChannel{Kind: channelTarball, Target: exe}
	}

	return updateChannel{Kind: channelUnknown}
}

func envFlagEnabled(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "1", "true", "yes", "on":
		return true
	default:
		return false
	}
}

// appBundleRoot walks up from a macOS executable at
// <root>/Meron.app/Contents/MacOS/meron and returns the <root>/Meron.app dir.
// Empty if the executable is not inside a .app bundle (e.g. `go run`).
func appBundleRoot(exe string) string {
	dir := filepath.Dir(exe) // .../Contents/MacOS
	for i := 0; i < 3 && dir != "" && dir != "/" && dir != "."; i++ {
		if strings.HasSuffix(dir, ".app") {
			return dir
		}
		dir = filepath.Dir(dir)
	}
	return ""
}
