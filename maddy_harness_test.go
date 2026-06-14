//go:build integration

package main

// Test harness that runs a real mail server (maddy, via Docker) and the Rust
// sidecar so the integration tests can exercise the full IMAP/SMTP/store path.
// Build the sidecar first: `cargo build --manifest-path meron-core/Cargo.toml`,
// then run with `go test -tags integration .`.

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

const (
	maddyImage    = "foxcpp/maddy:0.8"
	testPassword  = "testpass"
	maddyHostname = "maddy.test"
)

// Minimal maddy config: IMAP + submission on fixed container ports, TLS off
// (the sidecar connects with tls:false), local sqlite auth and storage.
const maddyConf = `hostname maddy.test
state_dir /data
runtime_dir /tmp/maddy-run

tls off

auth.pass_table local_authdb {
    table sql_table {
        driver sqlite3
        dsn credentials.db
        table_name passwords
    }
}

storage.imapsql local_mailboxes {
    driver sqlite3
    dsn imapsql.db
}

imap tcp://0.0.0.0:143 {
    insecure_auth yes
    auth &local_authdb
    storage &local_mailboxes
}

submission tcp://0.0.0.0:587 {
    insecure_auth yes
    auth &local_authdb
    deliver_to &local_mailboxes
}
`

type maddyServer struct {
	container string
	imapPort  int
	smtpPort  int
}

func freePort(t *testing.T) int {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("freePort: %v", err)
	}
	defer listener.Close()
	return listener.Addr().(*net.TCPAddr).Port
}

func dockerBin(t *testing.T) string {
	t.Helper()
	bin, err := exec.LookPath("docker")
	if err != nil {
		t.Skip("docker not found on PATH; skipping maddy integration tests")
	}
	return bin
}

// runCmd returns the command's stdout only — `docker run` writes image pull
// progress to stderr, which must not pollute the captured container ID.
func runCmd(t *testing.T, name string, args ...string) string {
	t.Helper()
	var stderr strings.Builder
	cmd := exec.Command(name, args...)
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("%s %s: %v\n%s%s", name, strings.Join(args, " "), err, out, stderr.String())
	}
	return strings.TrimSpace(string(out))
}

// startMaddy launches a throwaway maddy container with two test accounts
// (alice@maddy.test, bob@maddy.test) and waits for the IMAP greeting.
func startMaddy(t *testing.T) *maddyServer {
	t.Helper()
	docker := dockerBin(t)

	confPath := filepath.Join(t.TempDir(), "maddy.conf")
	if err := os.WriteFile(confPath, []byte(maddyConf), 0o644); err != nil {
		t.Fatalf("write maddy.conf: %v", err)
	}

	server := &maddyServer{imapPort: freePort(t), smtpPort: freePort(t)}
	// ":Z" relabels the bind mount for SELinux hosts (Fedora etc.); harmless elsewhere.
	server.container = runCmd(t, docker, "run", "-d", "--rm",
		"-v", confPath+":/data/maddy.conf:ro,Z",
		"-p", fmt.Sprintf("127.0.0.1:%d:143", server.imapPort),
		"-p", fmt.Sprintf("127.0.0.1:%d:587", server.smtpPort),
		maddyImage, "run")
	t.Cleanup(func() {
		out, err := exec.Command(docker, "stop", server.container).CombinedOutput()
		if err != nil {
			t.Logf("docker stop %s: %v\n%s", server.container, err, out)
		}
	})

	waitForIMAPGreeting(t, server.imapPort, docker, server.container)

	for _, user := range []string{"alice@" + maddyHostname, "bob@" + maddyHostname} {
		runCmd(t, docker, "exec", server.container, "maddy", "creds", "create", "--password", testPassword, user)
		runCmd(t, docker, "exec", server.container, "maddy", "imap-acct", "create", user)
	}
	return server
}

func waitForIMAPGreeting(t *testing.T, port int, docker, container string) {
	t.Helper()
	deadline := time.Now().Add(30 * time.Second)
	for time.Now().Before(deadline) {
		conn, err := net.DialTimeout("tcp", fmt.Sprintf("127.0.0.1:%d", port), time.Second)
		if err == nil {
			conn.SetReadDeadline(time.Now().Add(2 * time.Second))
			buf := make([]byte, 64)
			n, _ := conn.Read(buf)
			conn.Close()
			if strings.HasPrefix(string(buf[:n]), "* OK") {
				return
			}
		}
		time.Sleep(300 * time.Millisecond)
	}
	logs, _ := exec.Command(docker, "logs", container).CombinedOutput()
	t.Fatalf("maddy did not become ready on port %d\ncontainer logs:\n%s", port, logs)
}

// startSidecar launches the Rust core against a throwaway profile dir. The
// sidecar resolves its DB/media paths from XDG dirs (sidecarEnv), so pointing
// XDG_* and HOME at a temp dir isolates it; MERON_KEYRING=off keeps the test
// run out of the OS keychain.
func startSidecar(t *testing.T) *Sidecar {
	t.Helper()
	bin := "meron-core/target/debug/meron-core"
	if _, err := os.Stat(bin); err != nil {
		t.Fatalf("sidecar binary missing at %s — run `cargo build --manifest-path meron-core/Cargo.toml` first", bin)
	}

	profile := t.TempDir()
	t.Setenv("HOME", profile)
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(profile, "config"))
	t.Setenv("XDG_CACHE_HOME", filepath.Join(profile, "cache"))
	t.Setenv("MERON_KEYRING", "off")

	sidecar := NewSidecar(bin, os.Stderr)
	if err := sidecar.Start(nil); err != nil {
		t.Fatalf("start sidecar: %v", err)
	}
	t.Cleanup(sidecar.Close)
	return sidecar
}

// connectAccount registers an account on the sidecar against the local maddy.
func connectAccount(t *testing.T, sidecar *Sidecar, server *maddyServer, id, user string) {
	t.Helper()
	_, err := sidecar.Call("account.connect", map[string]any{
		"account":   id,
		"email":     user,
		"host":      "127.0.0.1",
		"port":      server.imapPort,
		"tls":       false,
		"smtp_host": "127.0.0.1",
		"smtp_port": server.smtpPort,
		"smtp_tls":  false,
		"user":      user,
		"password":  testPassword,
		"validate":  true,
	})
	if err != nil {
		t.Fatalf("account.connect %s: %v", id, err)
	}
}

// callMap invokes a sidecar method and asserts the result is a JSON object.
func callMap(t *testing.T, sidecar *Sidecar, method string, params map[string]any) map[string]any {
	t.Helper()
	result, err := sidecar.Call(method, params)
	if err != nil {
		t.Fatalf("%s: %v", method, err)
	}
	object, ok := result.(map[string]any)
	if !ok {
		t.Fatalf("%s: expected object result, got %T", method, result)
	}
	return object
}
