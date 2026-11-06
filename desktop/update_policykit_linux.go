package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// The script is fixed; the target is passed as an argument, not shell code.
// Only this short-lived helper runs as root. No GUI, archive extraction, or
// relaunch runs with elevated privileges. Use pkexec's default authorization
// policy: never install a rule granting passwordless access to this shell.
const policyKitUpdateScript = `set -eu
PATH=/usr/bin:/bin
export PATH
staging=
finish() {
    status=$?
    trap - 0
    if [ -n "$staging" ]; then
        rm -f -- "$staging" || :
    fi
    # Reserve these statuses for pkexec's authorization failures.
    case "$status" in 126|127) status=1 ;; esac
    exit "$status"
}
trap finish 0
trap 'exit 1' HUP INT TERM
target=$1
if [ ! -f "$target" ] || [ -L "$target" ]; then
    echo 'The installed application is no longer a regular file.' >&2
    exit 1
fi
parent=${target%/*}
staging=$(mktemp "$parent/.meron-update-XXXXXXXXXX")
cat > "$staging"
chown --reference="$target" -- "$staging"
chmod 755 -- "$staging"
mv -fT -- "$staging" "$target"
`

func installWithPolicyKit(payload, target string) error {
	if !filepath.IsAbs(target) {
		return fmt.Errorf("update: installation path must be absolute: %s", target)
	}
	pkexec, err := exec.LookPath("pkexec")
	if err != nil {
		return fmt.Errorf("update: administrator authorization requires PolicyKit (pkexec); install it or install the update manually: %w", err)
	}
	input, err := os.Open(payload)
	if err != nil {
		return err
	}
	defer input.Close()

	// A GUI app has no terminal agent. Disable pkexec's textual fallback so
	// authentication cannot consume the binary stream on stdin or hang unseen.
	cmd := exec.Command(pkexec, "--disable-internal-agent", "/bin/sh", "-c", policyKitUpdateScript, "meron-update", target)
	cmd.Stdin = input
	output, err := cmd.CombinedOutput()
	if err == nil {
		return nil
	}
	if exit, ok := err.(*exec.ExitError); ok {
		switch exit.ExitCode() {
		case 126:
			return fmt.Errorf("update: administrator authorization was cancelled; retry the update to try again")
		case 127:
			return fmt.Errorf("update: administrator authorization was denied or could not be obtained; check administrator permissions and the desktop PolicyKit authentication agent, or install the update manually: %s", strings.TrimSpace(string(output)))
		}
	}
	return fmt.Errorf("update: administrator update failed: %w: %s", err, strings.TrimSpace(string(output)))
}
