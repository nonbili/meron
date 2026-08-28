// Command obfuscate encodes Google OAuth credentials into the base64 blobs
// embedded in oauth_config.go.
//
// Usage:
//
//	go run ./cmd/obfuscate                         # prompts for both values
//	go run ./cmd/obfuscate <client-id> <secret>    # non-interactive
//
// Paste the printed lines over googleClientIDObf / googleClientSecretObf.
package main

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"meron/internal/obf"
)

func main() {
	var id, secret string
	if len(os.Args) >= 3 {
		id, secret = os.Args[1], os.Args[2]
	} else {
		r := bufio.NewReader(os.Stdin)
		fmt.Print("Google client ID: ")
		id = readLine(r)
		fmt.Print("Google client secret: ")
		secret = readLine(r)
	}
	if id == "" || secret == "" {
		fmt.Fprintln(os.Stderr, "both client id and secret are required")
		os.Exit(1)
	}
	fmt.Printf("\n\tgoogleClientIDObf     = %q\n\tgoogleClientSecretObf = %q\n", obf.Encode(id), obf.Encode(secret))
}

func readLine(r *bufio.Reader) string {
	s, _ := r.ReadString('\n')
	return strings.TrimSpace(s)
}
