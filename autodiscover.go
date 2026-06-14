package main

import (
	"context"
	"encoding/xml"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"sort"
	"strings"
	"time"
)

// DiscoveredConfig is the result of probing a domain for IMAP/SMTP settings.
// Ports already account for the socket type (993/465 implicit TLS, 143/587
// STARTTLS), so the frontend can prefill the form directly.
type DiscoveredConfig struct {
	IMAPHost string `json:"imap_host"`
	IMAPPort uint16 `json:"imap_port"`
	SMTPHost string `json:"smtp_host"`
	SMTPPort uint16 `json:"smtp_port"`
	Username string `json:"username"`
	Provider string `json:"provider_name,omitempty"`
	Source   string `json:"source"` // autoconfig | thunderbird | srv | guess
	// AppPasswordHint is set when the resolved provider requires an
	// app-specific password for IMAP/SMTP (no OAuth, and the account password
	// won't work). nil for ordinary providers.
	AppPasswordHint *AppPasswordHint `json:"app_password_hint,omitempty"`
}

// AppPasswordHint points the user at where to generate an app-specific password
// for a provider that rejects the regular account password over IMAP/SMTP.
type AppPasswordHint struct {
	Provider string `json:"provider"`
	URL      string `json:"url"`
}

// appPasswordHint recognizes providers that require an app-specific password.
// It matches on the resolved IMAP host first (robust across a provider's many
// vanity domains, e.g. every Yahoo ccTLD lands on mail.yahoo.com), then falls
// back to the email domain.
func appPasswordHint(imapHost, domain string) *AppPasswordHint {
	hostOrDomain := func(needles ...string) bool {
		for _, n := range needles {
			if strings.Contains(imapHost, n) || strings.Contains(domain, n) {
				return true
			}
		}
		return false
	}
	switch {
	case hostOrDomain("mail.me.com", "icloud.com", "me.com", "mac.com"):
		return &AppPasswordHint{Provider: "iCloud", URL: "https://account.apple.com/account/manage"}
	case hostOrDomain("fastmail"):
		return &AppPasswordHint{Provider: "Fastmail", URL: "https://app.fastmail.com/settings/security/apppassword"}
	case hostOrDomain("yahoo", "ymail", "rocketmail"):
		return &AppPasswordHint{Provider: "Yahoo", URL: "https://login.yahoo.com/account/security/app-passwords"}
	case hostOrDomain("aol.com"):
		return &AppPasswordHint{Provider: "AOL", URL: "https://login.aol.com/account/security/app-passwords"}
	default:
		return nil
	}
}

// mozilla autoconfig XML schema (subset we use).
type clientConfig struct {
	XMLName       xml.Name `xml:"clientConfig"`
	EmailProvider struct {
		ID          string `xml:"id,attr"`
		DisplayName string `xml:"displayName"`
		Incoming    []struct {
			Type       string `xml:"type,attr"`
			Hostname   string `xml:"hostname"`
			Port       uint16 `xml:"port"`
			SocketType string `xml:"socketType"`
			Username   string `xml:"username"`
		} `xml:"incomingServer"`
		Outgoing []struct {
			Type       string `xml:"type,attr"`
			Hostname   string `xml:"hostname"`
			Port       uint16 `xml:"port"`
			SocketType string `xml:"socketType"`
			Username   string `xml:"username"`
		} `xml:"outgoingServer"`
	} `xml:"emailProvider"`
}

// autodiscover resolves the email's domain into IMAP/SMTP settings. It tries,
// in order: provider-hosted Mozilla autoconfig, the Thunderbird ISPDB, DNS SRV
// records (RFC 6186), and finally common-prefix guessing. The first method that
// yields a usable config wins.
func (a *App) autodiscover(payload map[string]any) (any, error) {
	var req struct {
		Email string `json:"email"`
	}
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	email := strings.TrimSpace(req.Email)
	at := strings.LastIndex(email, "@")
	if at < 0 || at == len(email)-1 {
		return nil, errors.New("invalid email")
	}
	domain := strings.ToLower(email[at+1:])

	var cfg *DiscoveredConfig
	if c := a.discoverAutoconfig(email, domain); c != nil {
		cfg = c
		a.logf("autodiscover: %s via %s (imap=%s:%d smtp=%s:%d)", domain, cfg.Source, cfg.IMAPHost, cfg.IMAPPort, cfg.SMTPHost, cfg.SMTPPort)
	} else if c := a.discoverSRV(email, domain); c != nil {
		cfg = c
		a.logf("autodiscover: %s via srv (imap=%s:%d smtp=%s:%d)", domain, cfg.IMAPHost, cfg.IMAPPort, cfg.SMTPHost, cfg.SMTPPort)
	} else {
		cfg = &DiscoveredConfig{
			IMAPHost: "imap." + domain,
			IMAPPort: 993,
			SMTPHost: "smtp." + domain,
			SMTPPort: 465,
			Username: email,
			Source:   "guess",
		}
		a.logf("autodiscover: %s falling back to guess", domain)
	}
	cfg.AppPasswordHint = appPasswordHint(strings.ToLower(cfg.IMAPHost), domain)
	return cfg, nil
}

// discoverAutoconfig fetches and parses a Mozilla autoconfig document. It probes
// the provider-hosted URLs first (which can return account-specific settings)
// and then the central Thunderbird database.
func (a *App) discoverAutoconfig(email, domain string) *DiscoveredConfig {
	urls := []struct {
		url    string
		source string
	}{
		{fmt.Sprintf("https://autoconfig.%s/mail/config-v1.1.xml?emailaddress=%s", domain, email), "autoconfig"},
		{fmt.Sprintf("https://%s/.well-known/autoconfig/mail/config-v1.1.xml?emailaddress=%s", domain, email), "autoconfig"},
		{fmt.Sprintf("https://autoconfig.thunderbird.net/v1.1/%s", domain), "thunderbird"},
	}
	for _, u := range urls {
		cfg := fetchAutoconfig(u.url, email)
		if cfg != nil {
			cfg.Source = u.source
			return cfg
		}
	}
	return nil
}

func fetchAutoconfig(rawURL, email string) *DiscoveredConfig {
	client := &http.Client{Timeout: 6 * time.Second}
	req, err := http.NewRequest(http.MethodGet, rawURL, nil)
	if err != nil {
		return nil
	}
	req.Header.Set("User-Agent", "Meron-Mail-Autoconfig")
	res, err := client.Do(req)
	if err != nil {
		return nil
	}
	defer res.Body.Close()
	if res.StatusCode != http.StatusOK {
		return nil
	}
	body, err := io.ReadAll(io.LimitReader(res.Body, 1<<20))
	if err != nil {
		return nil
	}
	var doc clientConfig
	if err := xml.Unmarshal(body, &doc); err != nil {
		return nil
	}

	cfg := &DiscoveredConfig{Username: email, Provider: doc.EmailProvider.DisplayName}
	for _, in := range doc.EmailProvider.Incoming {
		if strings.EqualFold(in.Type, "imap") && in.Hostname != "" {
			cfg.IMAPHost = in.Hostname
			cfg.IMAPPort = portFor(in.Port, in.SocketType, true)
			if u := expandUsername(in.Username, email); u != "" {
				cfg.Username = u
			}
			break
		}
	}
	for _, out := range doc.EmailProvider.Outgoing {
		if strings.EqualFold(out.Type, "smtp") && out.Hostname != "" {
			cfg.SMTPHost = out.Hostname
			cfg.SMTPPort = portFor(out.Port, out.SocketType, false)
			break
		}
	}
	if cfg.IMAPHost == "" || cfg.SMTPHost == "" {
		return nil
	}
	return cfg
}

// discoverSRV implements RFC 6186 service discovery. _imaps/_submissions point
// to implicit-TLS endpoints; _imap/_submission to STARTTLS ones. A target of "."
// means the service is explicitly not offered.
func (a *App) discoverSRV(email, domain string) *DiscoveredConfig {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	resolver := net.DefaultResolver

	imapHost, imapPort := lookupSRV(ctx, resolver, "imaps", domain)
	if imapHost == "" {
		imapHost, imapPort = lookupSRV(ctx, resolver, "imap", domain)
	}
	smtpHost, smtpPort := lookupSRV(ctx, resolver, "submissions", domain)
	if smtpHost == "" {
		smtpHost, smtpPort = lookupSRV(ctx, resolver, "submission", domain)
	}
	if imapHost == "" || smtpHost == "" {
		return nil
	}
	return &DiscoveredConfig{
		IMAPHost: imapHost,
		IMAPPort: imapPort,
		SMTPHost: smtpHost,
		SMTPPort: smtpPort,
		Username: email,
		Source:   "srv",
	}
}

func lookupSRV(ctx context.Context, r *net.Resolver, service, domain string) (string, uint16) {
	_, addrs, err := r.LookupSRV(ctx, service, "tcp", domain)
	if err != nil || len(addrs) == 0 {
		return "", 0
	}
	sort.Slice(addrs, func(i, j int) bool {
		if addrs[i].Priority != addrs[j].Priority {
			return addrs[i].Priority < addrs[j].Priority
		}
		return addrs[i].Weight > addrs[j].Weight
	})
	best := addrs[0]
	target := strings.TrimSuffix(best.Target, ".")
	if target == "" || target == "." {
		return "", 0
	}
	return target, best.Port
}

// portFor returns the supplied port, or a sensible default derived from the
// socket type. imap controls which protocol's defaults are used.
func portFor(port uint16, socketType string, imap bool) uint16 {
	if port != 0 {
		return port
	}
	switch strings.ToUpper(socketType) {
	case "STARTTLS":
		if imap {
			return 143
		}
		return 587
	case "PLAIN":
		if imap {
			return 143
		}
		return 25
	default: // SSL / unspecified
		if imap {
			return 993
		}
		return 465
	}
}

// expandUsername resolves the autoconfig username placeholders against the
// address the user typed.
func expandUsername(template, email string) string {
	if template == "" {
		return ""
	}
	local := email
	if at := strings.LastIndex(email, "@"); at >= 0 {
		local = email[:at]
	}
	r := strings.NewReplacer("%EMAILADDRESS%", email, "%EMAILLOCALPART%", local)
	return r.Replace(template)
}
