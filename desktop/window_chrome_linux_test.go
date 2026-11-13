//go:build linux && !bindings

package main

import (
	"context"
	"errors"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// Exercise the actual native startup hooks in fresh processes. Go command
// tests cannot catch GTK crashes caused by installing a titlebar too late.
// Local runs opt in; CI provides an isolated Xvfb display.
func TestNativeTitlebarStartup(t *testing.T) {
	if os.Getenv("MERON_TEST_NATIVE_TITLEBAR") != "1" {
		t.Skip("set MERON_TEST_NATIVE_TITLEBAR=1 to run native display tests")
	}
	var backends []string
	if os.Getenv("DISPLAY") != "" {
		backends = append(backends, "x11")
	}
	if os.Getenv("WAYLAND_DISPLAY") != "" {
		backends = append(backends, "wayland")
	}
	if len(backends) == 0 {
		if os.Getenv("MERON_REQUIRE_NATIVE_DISPLAY") == "1" {
			t.Fatal("native display tests require an X11 or Wayland display")
		}
		t.Skip("requires an X11 or Wayland display")
	}

	// Parse the comment attached to import "C", rather than assuming the
	// first block comment in the file is the cgo preamble.
	fileAST, err := parser.ParseFile(token.NewFileSet(), "window_chrome_linux.go", nil, parser.ParseComments)
	if err != nil {
		t.Fatal(err)
	}
	var preamble string
	for _, declaration := range fileAST.Decls {
		decl, ok := declaration.(*ast.GenDecl)
		if !ok || decl.Tok != token.IMPORT || decl.Doc == nil {
			continue
		}
		for _, spec := range decl.Specs {
			if spec.(*ast.ImportSpec).Path.Value != `"C"` {
				continue
			}
			for _, comment := range decl.Doc.List {
				preamble += strings.TrimSuffix(strings.TrimPrefix(comment.Text, "/*"), "*/") + "\n"
			}
		}
	}
	if preamble == "" {
		t.Fatal("missing C preamble")
	}
	preamble = strings.ReplaceAll(preamble, "#cgo pkg-config: gtk+-3.0", "")
	harness, err := os.ReadFile("testdata/window_chrome_linux.c")
	if err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	file, binary := filepath.Join(dir, "titlebar.c"), filepath.Join(dir, "titlebar")
	if err := os.WriteFile(file, []byte(preamble+string(harness)), 0o600); err != nil {
		t.Fatal(err)
	}
	flags, err := exec.Command("pkg-config", "--cflags", "--libs", "gtk+-3.0").Output()
	if err != nil {
		t.Fatal(err)
	}
	args := append([]string{file, "-o", binary}, strings.Fields(string(flags))...)
	if output, err := exec.Command("cc", args...).CombinedOutput(); err != nil {
		t.Fatalf("compile native harness: %v\n%s", err, output)
	}

	for _, backend := range backends {
		for _, mode := range []string{"system", "integrated", "missed-hook", "background"} {
			name := backend + "/" + mode
			var args []string
			if mode != "system" {
				args = []string{mode}
			}
			t.Run(name, func(t *testing.T) {
				ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
				defer cancel()
				command := exec.CommandContext(ctx, binary, args...)
				// Force server-side decorations on X11 to exercise the same
				// missing initial GTK titlebar as KDE's default Wayland mode.
				command.Env = append(os.Environ(), "GDK_BACKEND="+backend, "GTK_CSD=0", "G_DEBUG=")
				if output, err := command.CombinedOutput(); err != nil {
					var exit *exec.ExitError
					if errors.As(err, &exit) && exit.ExitCode() == 77 && os.Getenv("MERON_REQUIRE_NATIVE_DISPLAY") != "1" {
						t.Skipf("%s display is unavailable: %s", backend, output)
					}
					t.Fatalf("native startup: %v\n%s", err, output)
				}
			})
		}
	}
}
