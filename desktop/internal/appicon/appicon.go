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
func Render(source []byte, colour Colour) ([]byte, error) {
	decoded, err := png.Decode(bytes.NewReader(source))
	if err != nil {
		return nil, fmt.Errorf("appicon: decode: %w", err)
	}
	stops := ramp(colour)
	bounds := decoded.Bounds()
	out := image.NewNRGBA(bounds)
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			pixel := color.NRGBAModel.Convert(decoded.At(x, y)).(color.NRGBA)
			if pixel.A == 0 {
				continue
			}
			shade := at(stops, Position(pixel))
			out.SetNRGBA(x, y, color.NRGBA{R: channel(shade[0]), G: channel(shade[1]), B: channel(shade[2]), A: pixel.A})
		}
	}
	var encoded bytes.Buffer
	if err := png.Encode(&encoded, out); err != nil {
		return nil, fmt.Errorf("appicon: encode: %w", err)
	}
	return encoded.Bytes(), nil
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
