// Command iosicons derives the dark and tinted variants of the iOS app icon
// from the shipped artwork, so iOS 18 can restyle the icon with the home
// screen. They are image files in the app, so they are generated ahead of time
// and committed. (Android's themed icon is a vector: scripts/icons/glyph.py.)
//
// Run after the shipped artwork changes, from the repository root:
//
//	bun run generate:ios-icons
//
// or from the desktop directory as `go run ./cmd/iosicons [repo root]`; the
// root defaults to the parent directory.
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

	"meron/internal/appicon"
)

func main() {
	root := ".."
	if len(os.Args) > 1 {
		root = os.Args[1]
	}
	if err := run(root); err != nil {
		fmt.Fprintln(os.Stderr, "iosicons:", err)
		os.Exit(1)
	}
}

func run(root string) error {
	return ios(filepath.Join(root, "mobile", "ios", "Meron", "Assets.xcassets"), filepath.Join(root, "desktop", "build", "appicon.png"))
}

// tintedGrey is the colour the tinted variant is drawn in. iOS takes the
// image's brightness as the amount of the user's tint, so the flaps are light.
const tintedGrey = "#c4c4c4"

// ios adds the dark and tinted variants to the app icon set. The shipped icon
// is flattened onto white, so the envelope is taken from the desktop artwork,
// which has it on a transparent canvas, and placed where the shipped one sits.
//
// Dark is the envelope alone: iOS supplies the dark plate behind it. Tinted is
// the envelope in greys on black: iOS colours it with the tint the user picks.
func ios(assets, artwork string) error {
	set := filepath.Join(assets, "AppIcon.appiconset")
	shipped, err := load(filepath.Join(set, "AppIcon.png"))
	if err != nil {
		return err
	}
	place := extent(shipped, func(c color.NRGBA) bool { return c.R < 245 || c.G < 245 || c.B < 245 })
	if place.Empty() {
		return fmt.Errorf("the shipped iOS icon is blank")
	}
	size := shipped.Bounds().Size()

	envelope, err := load(artwork)
	if err != nil {
		return err
	}
	from := extent(envelope, func(c color.NRGBA) bool { return c.A > 8 })
	dark := image.NewNRGBA(image.Rect(0, 0, size.X, size.Y))
	draw(dark, place, envelope, float64(from.Min.X), float64(from.Min.Y), float64(from.Max.X), float64(from.Max.Y))
	if err := save(filepath.Join(set, "AppIcon-Dark.png"), dark); err != nil {
		return err
	}

	source, err := os.ReadFile(artwork)
	if err != nil {
		return err
	}
	grey, _ := appicon.ParseColour(tintedGrey)
	rendered, err := appicon.Render(source, grey, 0)
	if err != nil {
		return err
	}
	greyEnvelope, err := decode(artwork, rendered)
	if err != nil {
		return err
	}
	tinted := plate(size.X, size.Y, color.NRGBA{A: 0xff})
	draw(tinted, place, greyEnvelope, float64(from.Min.X), float64(from.Min.Y), float64(from.Max.X), float64(from.Max.Y))
	return save(filepath.Join(set, "AppIcon-Tinted.png"), tinted)
}

func load(path string) (*image.NRGBA, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return decode(path, data)
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
// rectangle of dst and composites it over what is there, which may be
// transparent. Each target pixel is
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
			at := image.Pt(target.Min.X+tx, target.Min.Y+ty)
			under := dst.NRGBAAt(at.X, at.Y)
			below := float64(under.A) / 255 * (1 - alpha)
			total := alpha + below
			mix := func(over float64, under uint8) uint8 {
				return uint8(math.Round((over/a*alpha + float64(under)*below) / total))
			}
			dst.SetNRGBA(at.X, at.Y, color.NRGBA{
				R: mix(r, under.R), G: mix(g, under.G), B: mix(b, under.B),
				A: uint8(math.Round(255 * total)),
			})
		}
	}
}
