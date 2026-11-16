package appicon

import (
	"bytes"
	"image"
	"image/color"
	"image/png"
	"testing"
)

func TestParseColour(t *testing.T) {
	for value, want := range map[string]string{
		"#1d6fe0": "#1d6fe0", "1D6FE0": "#1d6fe0", " #FFFFFF ": "#ffffff", "#000000": "#000000",
	} {
		colour, ok := ParseColour(value)
		if !ok || colour.Hex() != want {
			t.Errorf("ParseColour(%q) = %q, %v; want %q", value, colour.Hex(), ok, want)
		}
	}
	for _, value := range []string{"", "#fff", "blue", "#12345g", "#1234567"} {
		if _, ok := ParseColour(value); ok {
			t.Errorf("ParseColour(%q) should be rejected", value)
		}
	}
}

// artwork is a stand-in for the icon: a dark fold, a mid-tone flap, a light
// sheet, a half-transparent edge and an empty corner.
func artwork(t *testing.T) []byte {
	t.Helper()
	img := image.NewNRGBA(image.Rect(0, 0, 5, 1))
	img.SetNRGBA(0, 0, color.NRGBA{R: 60, G: 150, B: 40, A: 255})
	img.SetNRGBA(1, 0, color.NRGBA{R: 150, G: 215, B: 90, A: 255})
	img.SetNRGBA(2, 0, color.NRGBA{R: 240, G: 250, B: 225, A: 255})
	img.SetNRGBA(3, 0, color.NRGBA{R: 150, G: 215, B: 90, A: 120})
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		t.Fatal(err)
	}
	return buf.Bytes()
}

func pixels(t *testing.T, data []byte) *image.NRGBA {
	t.Helper()
	decoded, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	out := image.NewNRGBA(decoded.Bounds())
	for x := 0; x < decoded.Bounds().Dx(); x++ {
		out.Set(x, 0, decoded.At(x, 0))
	}
	return out
}

func luminance(c color.NRGBA) float64 {
	return 0.2126*float64(c.R) + 0.7152*float64(c.G) + 0.0722*float64(c.B)
}

func TestRenderKeepsTheDrawingAndChangesTheColour(t *testing.T) {
	blue, _ := ParseColour("#1d6fe0")
	rendered, err := Render(artwork(t), blue, 0)
	if err != nil {
		t.Fatal(err)
	}
	got := pixels(t, rendered)
	fold, flap, sheet := got.NRGBAAt(0, 0), got.NRGBAAt(1, 0), got.NRGBAAt(2, 0)
	// Light and dark stay where they were.
	if !(luminance(fold) < luminance(flap) && luminance(flap) < luminance(sheet)) {
		t.Fatalf("shading lost: fold %v, flap %v, sheet %v", fold, flap, sheet)
	}
	// And every shade is now a blue: blue is its strongest channel, green gone.
	for name, pixel := range map[string]color.NRGBA{"fold": fold, "flap": flap, "sheet": sheet} {
		if pixel.B < pixel.R || pixel.B < pixel.G {
			t.Errorf("%s is not blue: %v", name, pixel)
		}
	}
	if sheet.R < 200 {
		t.Errorf("the sheet should stay a pale tint, got %v", sheet)
	}
}

func TestRenderKeepsTransparency(t *testing.T) {
	red, _ := ParseColour("#d0342c")
	rendered, err := Render(artwork(t), red, 0)
	if err != nil {
		t.Fatal(err)
	}
	got := pixels(t, rendered)
	if alpha := got.NRGBAAt(3, 0).A; alpha != 120 {
		t.Errorf("edge alpha = %d, want 120", alpha)
	}
	if alpha := got.NRGBAAt(4, 0).A; alpha != 0 {
		t.Errorf("empty corner alpha = %d, want 0", alpha)
	}
}

func TestRenderWorksForGreysAndExtremes(t *testing.T) {
	for _, hex := range []string{"#808080", "#000000", "#ffffff"} {
		colour, _ := ParseColour(hex)
		rendered, err := Render(artwork(t), colour, 0)
		if err != nil {
			t.Fatalf("%s: %v", hex, err)
		}
		got := pixels(t, rendered)
		if luminance(got.NRGBAAt(0, 0)) > luminance(got.NRGBAAt(2, 0)) {
			t.Errorf("%s: fold brighter than sheet", hex)
		}
	}
}

func TestRenderRejectsBadSource(t *testing.T) {
	if _, err := Render([]byte("not a png"), Colour{}, 0); err == nil {
		t.Fatal("garbage should not render")
	}
}

func TestRenderShrinksToMaxSize(t *testing.T) {
	img := image.NewNRGBA(image.Rect(0, 0, 8, 6))
	// The left half is opaque, the right half empty: shrinking must not let
	// the empty pixels darken or thin out the opaque ones.
	for y := 0; y < 6; y++ {
		for x := 0; x < 4; x++ {
			img.SetNRGBA(x, y, color.NRGBA{R: 150, G: 215, B: 90, A: 255})
		}
	}
	img.SetNRGBA(4, 0, color.NRGBA{R: 150, G: 215, B: 90, A: 255})
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		t.Fatal(err)
	}
	blue, _ := ParseColour("#1d6fe0")
	full, err := Render(buf.Bytes(), blue, 0)
	if err != nil {
		t.Fatal(err)
	}
	small, err := Render(buf.Bytes(), blue, 3)
	if err != nil {
		t.Fatal(err)
	}
	if size := pixelsOf(t, full).Bounds().Size(); size != image.Pt(8, 6) {
		t.Fatalf("maxSize 0 resized to %v", size)
	}
	got := pixelsOf(t, small)
	if size := got.Bounds().Size(); size != image.Pt(2, 1) {
		t.Fatalf("size = %v, want 2x1", size)
	}
	reference := pixelsOf(t, full).NRGBAAt(0, 0)
	if left := got.NRGBAAt(0, 0); left != reference {
		t.Errorf("opaque block = %v, want %v", left, reference)
	}
	// One opaque pixel in sixteen keeps its colour and a sixteenth of the alpha.
	if right := got.NRGBAAt(1, 0); right.A != 16 || right.R != reference.R || right.G != reference.G || right.B != reference.B {
		t.Errorf("edge block = %v, want %v at alpha 16", right, reference)
	}
}

func pixelsOf(t *testing.T, data []byte) *image.NRGBA {
	t.Helper()
	decoded, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	return toNRGBA(decoded)
}
