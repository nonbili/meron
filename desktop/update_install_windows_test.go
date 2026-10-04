package main

import (
	"archive/zip"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestWindowsUpdateWaitsForExitWithoutConsole(t *testing.T) {
	dir := t.TempDir()
	exe := writeUpdateRelaunchProbe(t, dir)
	parent := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 2")
	if err := parent.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = parent.Wait() })

	started := time.Now()
	runUpdateScript(t, windowsUpdateScript("", exe, filepath.Join(dir, "error.txt"), parent.Process.Pid, 2*time.Minute))
	if time.Since(started) < 2*time.Second {
		t.Fatal("helper relaunched before the original process exited")
	}
	assertUpdateRelaunched(t, dir)
}

func TestWindowsUpdateInstallerFailureReopensApp(t *testing.T) {
	for _, mode := range []string{"FullLanguage", "ConstrainedLanguage"} {
		t.Run(mode, func(t *testing.T) {
			// Include brackets as well as Unicode and shell metacharacters.
			dir := filepath.Join(t.TempDir(), "Meron's メロン [beta] & $app")
			if err := os.Mkdir(dir, 0o755); err != nil {
				t.Fatal(err)
			}
			exe := writeUpdateRelaunchProbe(t, dir)
			errorPath := filepath.Join(dir, "error.txt")
			script := "$ExecutionContext.SessionState.LanguageMode = '" + mode + "'\n" + windowsUpdateScript(filepath.Join(dir, "missing.exe"), exe, errorPath, -1, 2*time.Minute)
			runUpdateScript(t, script)
			data, err := os.ReadFile(errorPath)
			if err != nil {
				t.Fatal(err)
			}
			if !strings.HasPrefix(strings.TrimPrefix(string(data), "\ufeff"), "Windows update failed:") {
				t.Fatalf("unexpected error: %q", data)
			}
			assertUpdateRelaunched(t, dir)
		})
	}
}

func TestWindowsUpdateFailureReportedAfterRestart(t *testing.T) {
	t.Setenv("APPDATA", t.TempDir())
	if err := os.MkdirAll(appConfigDir(), 0o755); err != nil {
		t.Fatal(err)
	}
	message := "Windows update failed: elevation cancelled"
	if err := os.WriteFile(windowsUpdateErrorPath(), []byte("\ufeff"+message+"\r\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	u := newUpdater(&App{})
	if u.state != updateStateError || u.errMessage != message || u.installError != message {
		t.Fatalf("failure was not restored: state=%q error=%q", u.state, u.errMessage)
	}
	if newUpdater(&App{}).state != updateStateIdle {
		t.Fatal("failure was reported more than once")
	}
}

func TestWindowsUpdateTimeoutRecordsFailure(t *testing.T) {
	dir := t.TempDir()
	errorPath := filepath.Join(dir, "error.txt")
	// The test process stays alive, so the helper must time out without
	// launching an installer or another app against the single-instance lock.
	script := windowsUpdateScript("", writeUpdateRelaunchProbe(t, dir), errorPath, os.Getpid(), 100*time.Millisecond)
	script = "$ExecutionContext.SessionState.LanguageMode = 'ConstrainedLanguage'\n" + script
	cmd := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", encodePowerShellCommand(script))
	if output, err := cmd.CombinedOutput(); err == nil {
		t.Fatalf("helper did not time out: %s", output)
	}
	data, err := os.ReadFile(errorPath)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), "Timed out waiting for Meron to exit") {
		t.Fatalf("unexpected error: %q", data)
	}
	if _, err := os.Stat(filepath.Join(dir, "restarted.txt")); !os.IsNotExist(err) {
		t.Fatal("helper relaunched while the old process was still running")
	}
}

func writeUpdateRelaunchProbe(t *testing.T, dir string) string {
	t.Helper()
	path := filepath.Join(dir, "app.cmd")
	if err := os.WriteFile(path, []byte("@echo restarted>\"%~dp0restarted.txt\"\r\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

func runUpdateScript(t *testing.T, script string) {
	t.Helper()
	cmd := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", encodePowerShellCommand(script))
	// Like the GUI app, this helper has no interactive console input.
	if output, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("update helper: %v\n%s", err, output)
	}
}

func assertUpdateRelaunched(t *testing.T, dir string) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if _, err := os.Stat(filepath.Join(dir, "restarted.txt")); err == nil {
			return
		}
		time.Sleep(50 * time.Millisecond)
	}
	t.Fatal("app was not relaunched")
}

func TestExtractExeFromZip(t *testing.T) {
	for _, name := range []string{"release/MERON.EXE", "README.txt"} {
		t.Run(name, func(t *testing.T) {
			dir := t.TempDir()
			archive := filepath.Join(dir, "update.zip")
			file, err := os.Create(archive)
			if err != nil {
				t.Fatal(err)
			}
			writer := zip.NewWriter(file)
			entry, err := writer.Create(name)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := entry.Write([]byte("new executable")); err != nil {
				t.Fatal(err)
			}
			if err := writer.Close(); err != nil {
				t.Fatal(err)
			}
			if err := file.Close(); err != nil {
				t.Fatal(err)
			}
			dest := filepath.Join(dir, "staged.exe")
			err = extractExeFromZip(archive, "meron.exe", dest)
			if name == "README.txt" {
				if err == nil {
					t.Fatal("archive without an executable was accepted")
				}
				if _, err := os.Stat(dest); !os.IsNotExist(err) {
					t.Fatal("missing executable left a staging file")
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			data, err := os.ReadFile(dest)
			if err != nil || string(data) != "new executable" {
				t.Fatalf("extracted payload = %q, error = %v", data, err)
			}
		})
	}
}
