package main

import "testing"

func TestSelectedTLSMode(t *testing.T) {
	trueValue := true
	falseValue := false
	tests := []struct {
		name        string
		legacyTLS   bool
		explicitTLS *bool
		starttls    *bool
		port        uint16
		wantTLS     bool
		wantStart   bool
	}{
		{"legacy implicit TLS", true, nil, nil, 993, true, false},
		{"legacy STARTTLS", true, nil, nil, 587, false, true},
		{"explicit implicit TLS", true, &trueValue, nil, 143, true, false},
		{"explicit STARTTLS", true, nil, &trueValue, 993, false, true},
		{"lone false STARTTLS keeps port inference", true, nil, &falseValue, 587, false, true},
		{"explicit plaintext", true, &falseValue, &falseValue, 993, false, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gotTLS, gotStart := selectedTLSMode(tt.legacyTLS, tt.explicitTLS, tt.starttls, tt.port)
			if gotTLS != tt.wantTLS || gotStart != tt.wantStart {
				t.Fatalf("selectedTLSMode() = (%v, %v), want (%v, %v)", gotTLS, gotStart, tt.wantTLS, tt.wantStart)
			}
		})
	}
}
