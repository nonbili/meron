//go:build darwin

package main

import (
	"bytes"
	"testing"
)

func TestDarwinTrayBadgeIsSeparateFromTemplate(t *testing.T) {
	setIcon, setTemplate, setBadge := traySetIcon, traySetTemplateIcon, traySetUnreadBadge
	t.Cleanup(func() {
		traySetIcon, traySetTemplateIcon, traySetUnreadBadge = setIcon, setTemplate, setBadge
	})
	var template, regular []byte
	var badge bool
	traySetIcon = func(icon []byte) { template, regular = nil, icon }
	traySetTemplateIcon = func(icon, fallback []byte) { template, regular = icon, fallback }
	traySetUnreadBadge = func(unread bool) { badge = unread }
	for _, state := range []struct{ unread, monochrome bool }{
		{true, true}, {false, true}, {true, true}, {true, false}, {false, false},
	} {
		if err := applyPlatformTrayIcon(state.unread, state.monochrome); err != nil {
			t.Fatal(err)
		}
		if badge != (state.unread && state.monochrome) {
			t.Fatalf("%+v: incorrect badge visibility", state)
		}
		if state.monochrome {
			if !bytes.Equal(template, trayIconMonoPNG) || !bytes.Equal(regular, trayIcon(false)) {
				t.Fatalf("%+v: template should contain only the base icon", state)
			}
		} else if template != nil || !bytes.Equal(regular, trayIcon(state.unread)) {
			t.Fatalf("%+v: incorrect colour icon", state)
		}
	}
}
