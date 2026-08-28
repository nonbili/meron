package obf

import "testing"

func TestRoundTrip(t *testing.T) {
	for _, s := range []string{
		"590402290962-example.apps.googleusercontent.com",
		"GOCSPX-testsecret123",
		"",
	} {
		if got := Decode(Encode(s)); got != s {
			t.Errorf("round-trip mismatch: %q -> %q", s, got)
		}
	}
	if got := Decode("not!!base64"); got != "" {
		t.Errorf("malformed input should yield empty, got %q", got)
	}
}
