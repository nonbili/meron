package appicon

import (
	"bytes"
	"image"
	"image/color"
	"image/png"
	"testing"
)

func TestParseStyle(t *testing.T) {
	for _, value := range []string{"default", "blue", "light", "dark", "auto"} {
		if style, ok := ParseStyle(value); !ok || string(style) != value {
			t.Fatalf("ParseStyle(%q) = %q, %v", value, style, ok)
		}
	}
	if style, ok := ParseStyle("neon"); ok || style != StyleDefault {
		t.Fatalf("ParseStyle(neon) = %q, %v", style, ok)
	}
}

func TestResolve(t *testing.T) {
	cases := []struct {
		style       Style
		dark, known bool
		want        Variant
	}{
		{StyleDefault, true, true, VariantColour},
		{StyleBlue, true, false, VariantBlue},
		{StyleLight, true, true, VariantLight},
		{StyleDark, false, true, VariantDark},
		{StyleAuto, false, true, VariantLight},
		{StyleAuto, true, true, VariantDark},
		// Unknown appearance must not guess a grey.
		{StyleAuto, true, false, VariantColour},
	}
	for _, c := range cases {
		if got := c.style.Resolve(c.dark, c.known); got != c.want {
			t.Errorf("%s.Resolve(dark=%v, known=%v) = %v, want %v", c.style, c.dark, c.known, got, c.want)
		}
	}
}

func testIcon(t *testing.T) []byte {
	t.Helper()
	img := image.NewNRGBA(image.Rect(0, 0, 3, 1))
	img.SetNRGBA(0, 0, color.NRGBA{R: 120, G: 220, B: 60, A: 255})  // artwork green
	img.SetNRGBA(1, 0, color.NRGBA{R: 240, G: 250, B: 230, A: 128}) // translucent highlight
	// (2,0) stays fully transparent.
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		t.Fatal(err)
	}
	return buf.Bytes()
}

func decode(t *testing.T, data []byte) *image.NRGBA {
	t.Helper()
	img, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	out := image.NewNRGBA(img.Bounds())
	for x := 0; x < img.Bounds().Dx(); x++ {
		out.Set(x, 0, img.At(x, 0))
	}
	return out
}

func TestRenderColourReturnsSource(t *testing.T) {
	source := testIcon(t)
	got, err := Render(source, VariantColour)
	if err != nil || !bytes.Equal(got, source) {
		t.Fatalf("colour variant must be the source unchanged (err=%v)", err)
	}
}

func TestRenderGreys(t *testing.T) {
	source := testIcon(t)
	var brightness [2]int
	for i, variant := range []Variant{VariantLight, VariantDark} {
		rendered, err := Render(source, variant)
		if err != nil {
			t.Fatal(err)
		}
		img := decode(t, rendered)
		green := img.NRGBAAt(0, 0)
		if spread := int(green.B) - int(green.R); spread < 0 || spread > 16 {
			t.Errorf("variant %v is not near-neutral: %+v", variant, green)
		}
		if green.A != 255 || img.NRGBAAt(1, 0).A != 128 || img.NRGBAAt(2, 0).A != 0 {
			t.Errorf("variant %v changed the alpha channel", variant)
		}
		if img.NRGBAAt(1, 0).G <= green.G {
			t.Errorf("variant %v lost the artwork's light/dark order", variant)
		}
		brightness[i] = int(green.G)
	}
	if brightness[0] <= brightness[1] {
		t.Errorf("light variant (%d) must be brighter than dark (%d)", brightness[0], brightness[1])
	}
}

func TestRenderBlue(t *testing.T) {
	rendered, err := Render(testIcon(t), VariantBlue)
	if err != nil {
		t.Fatal(err)
	}
	img := decode(t, rendered)
	body, sheet := img.NRGBAAt(0, 0), img.NRGBAAt(1, 0)
	if !(body.B > body.G && body.G > body.R) {
		t.Errorf("the body should be blue: %+v", body)
	}
	if sheet.R <= body.R || sheet.A != 128 || img.NRGBAAt(2, 0).A != 0 {
		t.Errorf("the highlight should stay lighter and keep its alpha: %+v", sheet)
	}
}

func TestRenderRejectsBadSource(t *testing.T) {
	if _, err := Render([]byte("not a png"), VariantDark); err == nil {
		t.Fatal("expected a decode error")
	}
}
