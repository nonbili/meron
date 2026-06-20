package main

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestAppPasswordHintRecognizesKnownProviders(t *testing.T) {
	tests := []struct {
		name     string
		host     string
		domain   string
		provider string
	}{
		{"icloud host", "mail.me.com", "example.com", "iCloud"},
		{"icloud domain", "imap.example.com", "icloud.com", "iCloud"},
		{"fastmail", "imap.fastmail.com", "example.com", "Fastmail"},
		{"yahoo", "imap.mail.yahoo.com", "example.com", "Yahoo"},
		{"ymail domain", "imap.example.com", "ymail.com", "Yahoo"},
		{"aol", "imap.aol.com", "example.com", "AOL"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := appPasswordHint(tt.host, tt.domain)
			if got == nil {
				t.Fatalf("appPasswordHint(%q, %q) = nil", tt.host, tt.domain)
			}
			if got.Provider != tt.provider || got.URL == "" {
				t.Fatalf("hint = %#v, want provider %q with URL", got, tt.provider)
			}
		})
	}
}

func TestAppPasswordHintReturnsNilForOrdinaryProvider(t *testing.T) {
	if got := appPasswordHint("imap.example.com", "example.com"); got != nil {
		t.Fatalf("appPasswordHint returned %#v, want nil", got)
	}
}

func TestPortForUsesExplicitPortAndSocketDefaults(t *testing.T) {
	tests := []struct {
		name       string
		port       uint16
		socketType string
		imap       bool
		want       uint16
	}{
		{"explicit", 1993, "STARTTLS", true, 1993},
		{"imap starttls", 0, "STARTTLS", true, 143},
		{"smtp starttls", 0, "STARTTLS", false, 587},
		{"imap plain", 0, "PLAIN", true, 143},
		{"smtp plain", 0, "PLAIN", false, 25},
		{"imap ssl default", 0, "SSL", true, 993},
		{"smtp ssl default", 0, "SSL", false, 465},
		{"imap unspecified default", 0, "", true, 993},
		{"smtp unspecified default", 0, "", false, 465},
		{"case insensitive", 0, "starttls", false, 587},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := portFor(tt.port, tt.socketType, tt.imap); got != tt.want {
				t.Fatalf("portFor(%d, %q, %v) = %d, want %d", tt.port, tt.socketType, tt.imap, got, tt.want)
			}
		})
	}
}

func TestExpandUsername(t *testing.T) {
	tests := []struct {
		template string
		email    string
		want     string
	}{
		{"", "user@example.com", ""},
		{"%EMAILADDRESS%", "user@example.com", "user@example.com"},
		{"%EMAILLOCALPART%", "user@example.com", "user"},
		{"mail/%EMAILLOCALPART%/%EMAILADDRESS%", "user@example.com", "mail/user/user@example.com"},
		{"%EMAILLOCALPART%", "not-an-address", "not-an-address"},
		{"literal", "user@example.com", "literal"},
	}

	for _, tt := range tests {
		if got := expandUsername(tt.template, tt.email); got != tt.want {
			t.Fatalf("expandUsername(%q, %q) = %q, want %q", tt.template, tt.email, got, tt.want)
		}
	}
}

func TestFetchAutoconfigParsesUsableXML(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.Header.Get("User-Agent"); got != "Meron-Mail-Autoconfig" {
			t.Fatalf("User-Agent = %q", got)
		}
		w.Header().Set("Content-Type", "application/xml")
		_, _ = w.Write([]byte(`<?xml version="1.0"?>
<clientConfig>
  <emailProvider id="example.com">
    <displayName>Example Mail</displayName>
    <incomingServer type="pop3">
      <hostname>pop.example.com</hostname>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>imap.example.com</hostname>
      <socketType>STARTTLS</socketType>
      <username>%EMAILLOCALPART%</username>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.example.com</hostname>
      <port>2525</port>
      <socketType>STARTTLS</socketType>
      <username>%EMAILADDRESS%</username>
    </outgoingServer>
  </emailProvider>
</clientConfig>`))
	}))
	defer server.Close()

	cfg := fetchAutoconfig(server.URL, "person@example.com")
	if cfg == nil {
		t.Fatal("fetchAutoconfig returned nil")
	}
	if cfg.Provider != "Example Mail" {
		t.Fatalf("Provider = %q", cfg.Provider)
	}
	if cfg.IMAPHost != "imap.example.com" || cfg.IMAPPort != 143 {
		t.Fatalf("IMAP = %s:%d", cfg.IMAPHost, cfg.IMAPPort)
	}
	if cfg.SMTPHost != "smtp.example.com" || cfg.SMTPPort != 2525 {
		t.Fatalf("SMTP = %s:%d", cfg.SMTPHost, cfg.SMTPPort)
	}
	if cfg.Username != "person" {
		t.Fatalf("Username = %q, want local part", cfg.Username)
	}
}

func TestFetchAutoconfigRejectsUnusableResponses(t *testing.T) {
	tests := []struct {
		name   string
		status int
		body   string
	}{
		{"not found", http.StatusNotFound, ""},
		{"bad xml", http.StatusOK, "<clientConfig>"},
		{"missing smtp", http.StatusOK, `<clientConfig><emailProvider><incomingServer type="imap"><hostname>imap.example.com</hostname></incomingServer></emailProvider></clientConfig>`},
		{"missing imap", http.StatusOK, `<clientConfig><emailProvider><outgoingServer type="smtp"><hostname>smtp.example.com</hostname></outgoingServer></emailProvider></clientConfig>`},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.WriteHeader(tt.status)
				_, _ = w.Write([]byte(tt.body))
			}))
			defer server.Close()

			if cfg := fetchAutoconfig(server.URL, "person@example.com"); cfg != nil {
				t.Fatalf("fetchAutoconfig returned %#v, want nil", cfg)
			}
		})
	}
}
