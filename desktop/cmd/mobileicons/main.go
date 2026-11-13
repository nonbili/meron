// Command mobileicons derives the alternative launcher icons of the mobile
// apps from their shipped artwork, with the same tones the desktop app uses at
// run time. Mobile launchers only show icons that are part of the package, so
// these are generated ahead of time and committed.
//
// Run from the desktop directory after the shipped artwork changes; the
// argument is the repository root and defaults to the parent directory:
//
//	go run ./cmd/mobileicons
package main

import (
	"bytes"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"math"
	"os"
	"path/filepath"
	"strings"

	"meron/internal/appicon"
)

// Mobile launcher icons are opaque: the envelope sits on a plate. The plate is
// not part of the artwork, so it gets a colour of its own per variant instead
// of going through the tone ramp.
var plates = []struct {
	variant appicon.Variant
	plate   color.NRGBA
}{
	{appicon.VariantBlue, color.NRGBA{0xff, 0xff, 0xff, 0xff}},
	{appicon.VariantLight, color.NRGBA{0xff, 0xff, 0xff, 0xff}},
	{appicon.VariantDark, color.NRGBA{0x1b, 0x1c, 0x1f, 0xff}},
}

func main() {
	root := ".."
	if len(os.Args) > 1 {
		root = os.Args[1]
	}
	if err := run(root); err != nil {
		fmt.Fprintln(os.Stderr, "mobileicons:", err)
		os.Exit(1)
	}
}

func run(root string) error {
	if err := android(filepath.Join(root, "mobile", "android", "src", "main", "res")); err != nil {
		return err
	}
	return ios(filepath.Join(root, "mobile", "ios", "Meron", "Assets.xcassets"), filepath.Join(root, "desktop", "build", "appicon.png"))
}

// android writes, per density, the adaptive foreground of each variant and the
// legacy icon older launchers use: the middle 72 of the foreground's 108dp on
// the plate, which is how the shipped legacy icon relates to its foreground.
func android(res string) error {
	foregrounds, err := filepath.Glob(filepath.Join(res, "mipmap-*", "ic_launcher_foreground.png"))
	if err != nil {
		return err
	}
	if len(foregrounds) == 0 {
		return fmt.Errorf("no launcher foregrounds under %s", res)
	}
	for _, path := range foregrounds {
		dir := filepath.Dir(path)
		legacy, err := load(filepath.Join(dir, "ic_launcher.png"))
		if err != nil {
			return err
		}
		size := legacy.Bounds().Dx()
		for _, p := range plates {
			foreground, err := render(path, p.variant)
			if err != nil {
				return err
			}
			name := "ic_launcher_" + p.variant.Name()
			if err := save(filepath.Join(dir, name+"_foreground.png"), foreground); err != nil {
				return err
			}
			full := float64(foreground.Bounds().Dx())
			icon := plate(size, size, p.plate)
			draw(icon, image.Rect(0, 0, size, size), foreground, full/6, full/6, full*5/6, full*5/6)
			for _, target := range []string{name + ".png", name + "_round.png"} {
				if err := save(filepath.Join(dir, target), icon); err != nil {
					return err
				}
			}
		}
	}
	return nil
}

// ios writes one icon set per variant next to AppIcon. The shipped icon is
// flattened onto white, so the envelope is taken from the desktop artwork,
// which has it on a transparent canvas, and placed where the shipped one sits.
func ios(assets, artwork string) error {
	shipped, err := load(filepath.Join(assets, "AppIcon.appiconset", "AppIcon.png"))
	if err != nil {
		return err
	}
	contents, err := os.ReadFile(filepath.Join(assets, "AppIcon.appiconset", "Contents.json"))
	if err != nil {
		return err
	}
	place := extent(shipped, func(c color.NRGBA) bool { return c.R < 245 || c.G < 245 || c.B < 245 })
	if place.Empty() {
		return fmt.Errorf("the shipped iOS icon is blank")
	}
	for _, p := range plates {
		envelope, err := render(artwork, p.variant)
		if err != nil {
			return err
		}
		from := extent(envelope, func(c color.NRGBA) bool { return c.A > 8 })
		icon := plate(shipped.Bounds().Dx(), shipped.Bounds().Dy(), p.plate)
		draw(icon, place, envelope, float64(from.Min.X), float64(from.Min.Y), float64(from.Max.X), float64(from.Max.Y))
		name := p.variant.Name()
		set := filepath.Join(assets, "AppIcon"+strings.ToUpper(name[:1])+name[1:]+".appiconset")
		if err := os.MkdirAll(set, 0o755); err != nil {
			return err
		}
		if err := os.WriteFile(filepath.Join(set, "Contents.json"), contents, 0o644); err != nil {
			return err
		}
		if err := save(filepath.Join(set, "AppIcon.png"), icon); err != nil {
			return err
		}
	}
	return nil
}

func load(path string) (*image.NRGBA, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return decode(path, data)
}

func render(path string, variant appicon.Variant) (*image.NRGBA, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	rendered, err := appicon.Render(data, variant)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	return decode(path, rendered)
}

func decode(path string, data []byte) (*image.NRGBA, error) {
	decoded, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	bounds := decoded.Bounds()
	out := image.NewNRGBA(image.Rect(0, 0, bounds.Dx(), bounds.Dy()))
	for y := 0; y < bounds.Dy(); y++ {
		for x := 0; x < bounds.Dx(); x++ {
			out.SetNRGBA(x, y, color.NRGBAModel.Convert(decoded.At(bounds.Min.X+x, bounds.Min.Y+y)).(color.NRGBA))
		}
	}
	return out, nil
}

func save(path string, img image.Image) error {
	var encoded bytes.Buffer
	if err := png.Encode(&encoded, img); err != nil {
		return err
	}
	return os.WriteFile(path, encoded.Bytes(), 0o644)
}

func plate(width, height int, fill color.NRGBA) *image.NRGBA {
	img := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			img.SetNRGBA(x, y, fill)
		}
	}
	return img
}

// extent is the smallest rectangle holding every pixel that matches.
func extent(img *image.NRGBA, matches func(color.NRGBA) bool) image.Rectangle {
	var found image.Rectangle
	bounds := img.Bounds()
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			if matches(img.NRGBAAt(x, y)) {
				found = found.Union(image.Rect(x, y, x+1, y+1))
			}
		}
	}
	return found
}

// draw scales the source rectangle (x0, y0)-(x1, y1) of src into the target
// rectangle of dst and composites it over what is there. Each target pixel is
// the area average of the source pixels it covers, which is all a downscale
// needs; alpha is premultiplied while averaging so edges do not darken.
func draw(dst *image.NRGBA, target image.Rectangle, src *image.NRGBA, x0, y0, x1, y1 float64) {
	stepX, stepY := (x1-x0)/float64(target.Dx()), (y1-y0)/float64(target.Dy())
	limit := src.Bounds()
	for ty := 0; ty < target.Dy(); ty++ {
		top, bottom := y0+float64(ty)*stepY, y0+float64(ty+1)*stepY
		for tx := 0; tx < target.Dx(); tx++ {
			left, right := x0+float64(tx)*stepX, x0+float64(tx+1)*stepX
			var r, g, b, a, area float64
			for sy := int(math.Floor(top)); float64(sy) < bottom; sy++ {
				coverY := math.Min(bottom, float64(sy+1)) - math.Max(top, float64(sy))
				for sx := int(math.Floor(left)); float64(sx) < right; sx++ {
					cover := coverY * (math.Min(right, float64(sx+1)) - math.Max(left, float64(sx)))
					if cover <= 0 {
						continue
					}
					area += cover
					if !image.Pt(sx, sy).In(limit) {
						continue
					}
					pixel := src.NRGBAAt(sx, sy)
					alpha := float64(pixel.A) / 255 * cover
					r += float64(pixel.R) * alpha
					g += float64(pixel.G) * alpha
					b += float64(pixel.B) * alpha
					a += alpha
				}
			}
			if area == 0 || a == 0 {
				continue
			}
			alpha := a / area
			under := dst.NRGBAAt(target.Min.X+tx, target.Min.Y+ty)
			mix := func(over float64, below uint8) uint8 {
				return uint8(math.Round(over/a*alpha + float64(below)*(1-alpha)))
			}
			dst.SetNRGBA(target.Min.X+tx, target.Min.Y+ty, color.NRGBA{R: mix(r, under.R), G: mix(g, under.G), B: mix(b, under.B), A: 0xff})
		}
	}
}
