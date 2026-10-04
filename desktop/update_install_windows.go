package main

import (
	"archive/zip"
	"encoding/base64"
	"encoding/binary"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"
	"unicode/utf16"
)

// oldExeSuffix marks the displaced copy of a running exe. Windows lets a
// running image be renamed but not overwritten, so the portable channel renames
// itself out of the way and the leftover is swept up on the next launch.
const oldExeSuffix = ".old"

// applyUpdate installs the downloaded payload and queues a relaunch. The caller
// quits immediately afterwards.
func applyUpdate(channel updateChannel, payload string) error {
	switch channel.Kind {
	case channelNSIS:
		return runSilentInstaller(payload, channel.Target)
	case channelPortable:
		return replacePortableExe(payload, channel.Target)
	default:
		return fmt.Errorf("update: unsupported channel %q", channel.Kind)
	}
}

// runSilentInstaller launches the NSIS installer in silent mode and starts the
// app again once it finishes. The installer may raise a UAC prompt for a
// per-machine install; the UI warns about that before this is called.
func runSilentInstaller(installer, exe string) error {
	if _, err := os.Stat(installer); err != nil {
		return fmt.Errorf("update: installer unavailable: %w", err)
	}
	return startWindowsUpdateHelper(installer, exe)
}

// PowerShell's process wait works without a console. cmd's timeout exits
// immediately with redirected stdin, racing the single-instance lock; starting
// NSIS before this process exits also races the locked executable. Keep the
// helper unelevated so the relaunched app runs as the original user, and elevate
// only NSIS (the Wails installer requires admin rights).
func startWindowsUpdateHelper(installer, exe string) error {
	script := windowsUpdateScript(installer, exe, windowsUpdateErrorPath(), os.Getpid(), 2*time.Minute)
	cmd := exec.Command("powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-EncodedCommand", encodePowerShellCommand(script))
	cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: createNoWindow | 0x00000200 /* CREATE_NEW_PROCESS_GROUP */}
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("update: could not start update helper: %w", err)
	}
	return cmd.Process.Release()
}

func windowsUpdateErrorPath() string {
	// Keep failures outside the download cache, which startup cleans up.
	return filepath.Join(appConfigDir(), "update-error.txt")
}

func pendingUpdateError() string {
	path := windowsUpdateErrorPath()
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	_ = os.Remove(path)
	// Windows PowerShell 5.1's UTF-8 Set-Content writes a BOM.
	return strings.TrimSpace(strings.TrimPrefix(string(data), "\ufeff"))
}

func powerShellQuote(value string) string {
	return "'" + strings.ReplaceAll(value, "'", "''") + "'"
}

func encodePowerShellCommand(script string) string {
	units := utf16.Encode([]rune(script))
	data := make([]byte, 2*len(units))
	for i, unit := range units {
		binary.LittleEndian.PutUint16(data[2*i:], unit)
	}
	return base64.StdEncoding.EncodeToString(data)
}

func windowsUpdateScript(installer, exe, errorPath string, pid int, wait time.Duration) string {
	install := ""
	if installer != "" {
		// NSIS /D must be last and its directory must not be quoted, even when
		// it contains spaces. Start-Process passes this one argument string on.
		install = fmt.Sprintf(`
    $installer = Start-Process -FilePath %s -ArgumentList %s -Verb RunAs -Wait -PassThru
    if ($installer.ExitCode -ne 0) { throw "Installer exited with code $($installer.ExitCode)" }
`, powerShellQuote(installer), powerShellQuote("/S /D="+filepath.Dir(exe)))
	}
	return fmt.Sprintf(`$ErrorActionPreference = 'Stop'
$exe = %s
$errorPath = %s
function Write-UpdateError([string]$message) {
    try {
        Set-Content -LiteralPath $errorPath -Value ("Windows update failed: " + $message) -Encoding UTF8
    } catch { }
}
$deadline = [DateTime]::UtcNow.AddMilliseconds(%d)
while (Get-Process -Id %d -ErrorAction SilentlyContinue) {
    if ([DateTime]::UtcNow -ge $deadline) {
        Write-UpdateError 'Timed out waiting for Meron to exit. Please restart Meron and try the update again.'
        exit 1
    }
    Start-Sleep -Milliseconds 200
}
try {
%s
} catch {
    Write-UpdateError $_.Exception.Message
}
# Reopen the existing app even if elevation was cancelled or installation failed.
try {
    # Use .NET directly: Start-Process resolves WorkingDirectory as a wildcard
    # path in Windows PowerShell, so brackets in a portable install can fail.
    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $exe
    $startInfo.WorkingDirectory = [System.IO.Path]::GetDirectoryName($exe)
    $startInfo.UseShellExecute = $true
    [void][System.Diagnostics.Process]::Start($startInfo)
} catch {
    # Constrained Language Mode blocks constructing ProcessStartInfo and
    # calling Process.Start, but permits Start-Process. Omit WorkingDirectory
    # here so bracketed paths do not go through its wildcard resolution.
    try {
        Start-Process -FilePath $exe
    } catch {
        Write-UpdateError $_.Exception.Message
    }
}
`, powerShellQuote(exe), powerShellQuote(errorPath), wait.Milliseconds(), pid, install)
}

// replacePortableExe swaps the loose meron.exe from the portable zip.
func replacePortableExe(archive, exe string) error {
	parent := filepath.Dir(exe)
	if err := ensureWritableDir(parent); err != nil {
		return fmt.Errorf("update: %s is not writable — install the update manually: %w", parent, err)
	}

	staging := filepath.Join(parent, fmt.Sprintf(".meron-update-%d.exe", os.Getpid()))
	_ = os.Remove(staging)
	if err := extractExeFromZip(archive, filepath.Base(exe), staging); err != nil {
		_ = os.Remove(staging)
		return err
	}

	previous := exe + oldExeSuffix
	_ = os.Remove(previous)
	if err := os.Rename(exe, previous); err != nil {
		_ = os.Remove(staging)
		return fmt.Errorf("update: could not move the old executable aside: %w", err)
	}
	if err := os.Rename(staging, exe); err != nil {
		_ = os.Rename(previous, exe)
		_ = os.Remove(staging)
		return fmt.Errorf("update: could not move the new executable into place: %w", err)
	}

	return startWindowsUpdateHelper("", exe)
}

func extractExeFromZip(archive, name, dest string) error {
	reader, err := zip.OpenReader(archive)
	if err != nil {
		return err
	}
	defer reader.Close()
	for _, entry := range reader.File {
		if entry.FileInfo().IsDir() || !strings.EqualFold(filepath.Base(entry.Name), name) {
			continue
		}
		source, err := entry.Open()
		if err != nil {
			return err
		}
		defer source.Close()
		out, err := os.OpenFile(dest, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o755)
		if err != nil {
			return err
		}
		if _, err := io.Copy(out, source); err != nil {
			out.Close()
			return err
		}
		return out.Close()
	}
	return fmt.Errorf("update: %s not found in the downloaded archive", name)
}

// sweepReplacedExecutable deletes the previous exe left behind by a portable
// update. Called on startup, once the new copy is the one running.
func sweepReplacedExecutable() {
	exe, err := os.Executable()
	if err != nil {
		return
	}
	_ = os.Remove(exe + oldExeSuffix)
}
