// Package appicon recolours the app icon's artwork.
//
// The icon is the envelope on a transparent canvas, drawn in shades of one
// green. A recoloured icon keeps the drawing (its folds, highlights and the
// paper inside) and swaps the green for another colour, so any colour the user
// picks gives a finished-looking icon. Nothing is stored as an image file: the
// result is computed from the shipped artwork and follows any change to it.
package appicon

import (
	"bytes"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"math"
	"strconv"
	"strings"
)

// Colour is an opaque sRGB colour, each channel in [0, 1].
type Colour [3]float64

// ParseColour reads "#rrggbb" (case-insensitive, the hash optional).
func ParseColour(value string) (Colour, bool) {
	hex := strings.TrimPrefix(strings.TrimSpace(value), "#")
	if len(hex) != 6 {
		return Colour{}, false
	}
	rgb, err := strconv.ParseUint(hex, 16, 32)
	if err != nil {
		return Colour{}, false
	}
	return Colour{float64(rgb>>16&0xff) / 255, float64(rgb>>8&0xff) / 255, float64(rgb&0xff) / 255}, true
}

// Hex is the colour as lowercase "#rrggbb".
func (c Colour) Hex() string {
	channel := func(value float64) int { return int(math.Round(255 * clamp(value))) }
	return fmt.Sprintf("#%02x%02x%02x", channel(c[0]), channel(c[1]), channel(c[2]))
}

// The artwork sits in the upper half of the luminance range, so these are
// stretched to cover the whole ramp.
const (
	artworkBlack = 0.45
	artworkWhite = 0.97
)

type stop struct {
	position float64
	colour   Colour
}

// ramp maps the artwork's luminance onto shades of one colour: a deep shade in
// the folds, the colour itself across the flaps, a light tint on the paper and
// almost white at the highlights. The picked colour lands where the shipped
// green is most visible, so the icon reads as that colour.
func ramp(c Colour) []stop {
	return []stop{
		{0, mix(c, Colour{0, 0, 0}, 0.48)},
		{0.55, c},
		{0.82, mix(c, Colour{1, 1, 1}, 0.50)},
		{1, mix(c, Colour{1, 1, 1}, 0.92)},
	}
}

// Render returns the PNG of the icon in source redrawn in shades of colour.
// With maxSize above zero, the result is halved until no side exceeds it:
// desktop icons are drawn at a few hundred pixels at most, and a quarter of the
// pixels takes a quarter of the time.
func Render(source []byte, colour Colour, maxSize int) ([]byte, error) {
	decoded, err := png.Decode(bytes.NewReader(source))
	if err != nil {
		return nil, fmt.Errorf("appicon: decode: %w", err)
	}
	img := toNRGBA(decoded)
	for maxSize > 0 && (img.Rect.Dx() > maxSize || img.Rect.Dy() > maxSize) && img.Rect.Dx() > 1 && img.Rect.Dy() > 1 {
		img = halve(img)
	}
	stops := ramp(colour)
	for i := 0; i+3 < len(img.Pix); i += 4 {
		pixel := color.NRGBA{R: img.Pix[i], G: img.Pix[i+1], B: img.Pix[i+2], A: img.Pix[i+3]}
		if pixel.A == 0 {
			continue
		}
		shade := at(stops, Position(pixel))
		img.Pix[i], img.Pix[i+1], img.Pix[i+2] = channel(shade[0]), channel(shade[1]), channel(shade[2])
	}
	// Most of the time otherwise goes to compression, and the PNG is only
	// handed to the system.
	var encoded bytes.Buffer
	encoder := png.Encoder{CompressionLevel: png.BestSpeed}
	if err := encoder.Encode(&encoded, img); err != nil {
		return nil, fmt.Errorf("appicon: encode: %w", err)
	}
	return encoded.Bytes(), nil
}

// toNRGBA returns a copy of img as NRGBA at the origin, which Render then
// edits in place. PNGs with an alpha channel decode as NRGBA already and are
// copied row by row; anything else goes through the colour model.
func toNRGBA(img image.Image) *image.NRGBA {
	bounds := img.Bounds()
	out := image.NewNRGBA(image.Rect(0, 0, bounds.Dx(), bounds.Dy()))
	if src, ok := img.(*image.NRGBA); ok {
		for y := 0; y < bounds.Dy(); y++ {
			from := src.PixOffset(bounds.Min.X, bounds.Min.Y+y)
			copy(out.Pix[y*out.Stride:y*out.Stride+4*bounds.Dx()], src.Pix[from:from+4*bounds.Dx()])
		}
		return out
	}
	for y := 0; y < bounds.Dy(); y++ {
		for x := 0; x < bounds.Dx(); x++ {
			out.SetNRGBA(x, y, color.NRGBAModel.Convert(img.At(bounds.Min.X+x, bounds.Min.Y+y)).(color.NRGBA))
		}
	}
	return out
}

// halve averages each 2×2 block into one pixel (an odd last row or column is
// dropped). Colours are weighted by alpha so transparent pixels do not darken
// the edges.
func halve(src *image.NRGBA) *image.NRGBA {
	width, height := src.Rect.Dx()/2, src.Rect.Dy()/2
	out := image.NewNRGBA(image.Rect(0, 0, width, height))
	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			var r, g, b, a int
			for _, offset := range [4]int{
				src.PixOffset(2*x, 2*y), src.PixOffset(2*x+1, 2*y),
				src.PixOffset(2*x, 2*y+1), src.PixOffset(2*x+1, 2*y+1),
			} {
				alpha := int(src.Pix[offset+3])
				r += int(src.Pix[offset]) * alpha
				g += int(src.Pix[offset+1]) * alpha
				b += int(src.Pix[offset+2]) * alpha
				a += alpha
			}
			if a == 0 {
				continue
			}
			i := out.PixOffset(x, y)
			out.Pix[i] = uint8((r + a/2) / a)
			out.Pix[i+1] = uint8((g + a/2) / a)
			out.Pix[i+2] = uint8((b + a/2) / a)
			out.Pix[i+3] = uint8((a + 2) / 4)
		}
	}
	return out
}

// Position is where a pixel of the artwork sits between its darkest fold (0)
// and its brightest highlight (1).
func Position(pixel color.NRGBA) float64 {
	luminance := (0.2126*float64(pixel.R) + 0.7152*float64(pixel.G) + 0.0722*float64(pixel.B)) / 255
	return clamp((luminance - artworkBlack) / (artworkWhite - artworkBlack))
}

// at interpolates the ramp at a position in [0, 1].
func at(stops []stop, position float64) Colour {
	for i := 1; i < len(stops); i++ {
		from, to := stops[i-1], stops[i]
		if position > to.position {
			continue
		}
		return mix(from.colour, to.colour, (position-from.position)/(to.position-from.position))
	}
	return stops[len(stops)-1].colour
}

func mix(from, to Colour, amount float64) Colour {
	var out Colour
	for i := range out {
		out[i] = from[i] + (to[i]-from[i])*amount
	}
	return out
}

func clamp(value float64) float64 { return math.Min(1, math.Max(0, value)) }

func channel(value float64) uint8 { return uint8(math.Round(255 * clamp(value))) }
