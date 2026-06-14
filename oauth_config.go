package main

import (
	"os"

	"meron/internal/obf"
)

const (
	// Loopback host for the OAuth redirect. Per RFC 8252 (OAuth for Native Apps)
	// we bind an ephemeral port (host:0) at flow start and build the redirect URI
	// from the OS-assigned port — both Google and Microsoft match loopback
	// redirects on host only, so the portless URI is what's registered. This
	// avoids hardcoding a port that may already be in use.
	oauthLoopbackHost = "127.0.0.1"

	// Delegated scopes for Outlook/Microsoft 365 IMAP+SMTP access. offline_access
	// yields a refresh token; openid/email/profile populate the id_token we read
	// the account email and name from.
	outlookScopes = "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access openid email profile"

	outlookTokenURL = "https://login.microsoftonline.com/common/oauth2/v2.0/token"
)

// Obfuscated Google OAuth credentials baked into release builds.
//
// These are intentionally embedded in source: the Flatpak is built on
// infrastructure we don't control, with no build-time secret injection, so the
// values have to travel with the public repo. Per Google's installed-app model
// the client secret is not confidential; the obfuscation (see internal/obf)
// only defeats plaintext scraping, not a determined extractor.
//
// Generate replacements with:  go run ./cmd/obfuscate
// then paste the printed values here.
var (
	googleClientIDObf     = "f1FFXVgbXFZAQFgZWwGpVyUNGAkfQAFVQRgJGE4G6At0ABwFDF8NFx0EBl1HH/tMPRZcCAFCCA0QARtIBFL1UjkAHBtATgAM"
	googleClientSecretObf = "CioxPD51QlcQNz0fIkvUSwoRQC4vaQM2QC4ydTtzw1Y0DQA="

	// Outlook is a public client (PKCE), so there is no client secret — only the
	// Application (client) ID. Generate the obfuscated value the same way as
	// Google (`go run ./cmd/obfuscate`) and paste it here for release builds.
	outlookClientIDObf = ""
)

// googleClientID returns the OAuth client id. A non-empty MERON_GOOGLE_CLIENT_ID
// env var wins, so local development can point straight at its own credentials
// without touching the baked-in defaults.
func googleClientID() string {
	if v := os.Getenv("MERON_GOOGLE_CLIENT_ID"); v != "" {
		return v
	}
	return obf.Decode(googleClientIDObf)
}

// googleClientSecret mirrors googleClientID for the client secret.
func googleClientSecret() string {
	if v := os.Getenv("MERON_GOOGLE_CLIENT_SECRET"); v != "" {
		return v
	}
	return obf.Decode(googleClientSecretObf)
}

func gmailOAuthConfigured() bool {
	return googleClientID() != "" && googleClientSecret() != ""
}

// outlookClientID returns the Microsoft OAuth client id. A non-empty
// MERON_OUTLOOK_CLIENT_ID env var wins, so local development can point at its
// own Azure app registration without touching the baked-in default.
func outlookClientID() string {
	if v := os.Getenv("MERON_OUTLOOK_CLIENT_ID"); v != "" {
		return v
	}
	if outlookClientIDObf == "" {
		return ""
	}
	return obf.Decode(outlookClientIDObf)
}

func outlookOAuthConfigured() bool {
	return outlookClientID() != ""
}
