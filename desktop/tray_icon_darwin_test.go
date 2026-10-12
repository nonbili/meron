//go:build darwin

package main

import (
	"bytes"
	"testing"
)

func TestDarwinTrayBadgeIsSeparateFromTemplate(t *testing.T) {
	setIcon, setTemplate := traySetIcon, traySetTemplateIconWithBadge
	t.Cleanup(func() {
		traySetIcon, traySetTemplateIconWithBadge = setIcon, setTemplate
	})
	var template, regular []byte
	var badge bool
	traySetIcon = func(icon []byte) { template, regular, badge = nil, icon, false }
	traySetTemplateIconWithBadge = func(icon []byte, unread bool) { template, regular, badge = icon, nil, unread }
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
			if !bytes.Equal(template, trayIconMonoPNG) || regular != nil {
				t.Fatalf("%+v: template should contain only the base icon", state)
			}
		} else if template != nil || !bytes.Equal(regular, trayIcon(state.unread)) {
			t.Fatalf("%+v: incorrect colour icon", state)
		}
	}
}
