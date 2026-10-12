//go:build (linux || freebsd || openbsd || netbsd) && !android

package systray

import (
	"bytes"
	"image"
	"image/color"
	"image/png"
	"testing"

	"github.com/godbus/dbus/v5"
)

func iconTestPNG(t *testing.T, shade color.RGBA) []byte {
	t.Helper()
	img := image.NewRGBA(image.Rect(0, 0, 1, 1))
	img.SetRGBA(0, 0, shade)
	var buffer bytes.Buffer
	if err := png.Encode(&buffer, img); err != nil {
		t.Fatal(err)
	}
	return buffer.Bytes()
}

func isolateIconState(t *testing.T) {
	t.Helper()
	previous, previousName, previousPath := instance, iconName, iconThemePath
	instance = &tray{menu: &menuLayout{}, menuVersion: 1}
	iconName, iconThemePath = "", ""
	t.Cleanup(func() { instance, iconName, iconThemePath = previous, previousName, previousPath })
}

func TestIconWithNameReplacesFallbackBeforePublication(t *testing.T) {
	isolateIconState(t)
	old := iconTestPNG(t, color.RGBA{R: 255, A: 255})
	current := iconTestPNG(t, color.RGBA{G: 255, A: 255})
	SetIconWithName(old, "", "")
	SetIconWithName(current, "current-symbolic", "/current")
	properties := instance.createPropSpec()["org.kde.StatusNotifierItem"]
	if properties["IconName"].Value != "current-symbolic" || properties["IconThemePath"].Value != "/current" {
		t.Fatal("recovery did not replace the name and path")
	}
	pixels := properties["IconPixmap"].Value.([]PX)
	if len(pixels) != 1 || !bytes.Equal(pixels[0].Pix, convertToPixels(current).Pix) {
		t.Fatal("recovery left a stale fallback pixmap")
	}
}

func TestClickHandlerIsIncludedInPublicationSnapshot(t *testing.T) {
	isolateIconState(t)
	left, right := tappedLeft, tappedRight
	t.Cleanup(func() { tappedLeft, tappedRight = left, right })
	SetOnTapped(func() {})
	properties := instance.createPropSpec()["org.kde.StatusNotifierItem"]
	if properties["ItemIsMenu"].Value != false {
		t.Fatal("publication advertised menu-only clicks despite the preinstalled handler")
	}
}

func TestIconWithNameReplacesAndClearsPublishedPixmap(t *testing.T) {
	isolateIconState(t)
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		t.Skip("run under dbus-run-session to check published properties")
	}
	defer conn.Close()
	if err := instance.publishProperties(conn); err != nil {
		t.Fatal(err)
	}
	old := iconTestPNG(t, color.RGBA{R: 255, A: 255})
	current := iconTestPNG(t, color.RGBA{G: 255, A: 255})
	SetIconWithName(old, "", "")
	SetIconWithName(current, "current-symbolic", "/current")
	values, dbusErr := instance.props.GetAll("org.kde.StatusNotifierItem")
	if dbusErr != nil {
		t.Fatal(dbusErr)
	}
	if values["IconName"].Value() != "current-symbolic" || values["IconThemePath"].Value() != "/current" {
		t.Fatal("published name/path was not replaced")
	}
	pixels := values["IconPixmap"].Value().([]PX)
	if len(pixels) != 1 || !bytes.Equal(pixels[0].Pix, convertToPixels(current).Pix) {
		t.Fatal("published pixmap was not replaced")
	}
	SetIcon(nil)
	value, dbusErr := instance.props.Get("org.kde.StatusNotifierItem", "IconPixmap")
	if dbusErr != nil || len(value.Value().([]PX)) != 0 {
		t.Fatal("SetIcon(nil) did not clear the published pixmap")
	}
	SetIconWithName(old, "", "")
	SetIconWithName(nil, "current-symbolic", "/current")
	value, dbusErr = instance.props.Get("org.kde.StatusNotifierItem", "IconPixmap")
	if dbusErr != nil || len(value.Value().([]PX)) != 0 {
		t.Fatal("SetIconWithName(nil) did not clear the published pixmap")
	}
}
