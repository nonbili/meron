// Package appicon derives the alternative app icons from the shipped artwork.
//
// The variants are computed from the colour icon rather than stored as files:
// they follow any future change to the artwork and add nothing to the bundle.
package appicon

import (
	"bytes"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"math"
)

// Style is the user's choice. Auto resolves to Light or Dark with the system.
type Style string

const (
	StyleDefault Style = "default"
	StyleBlue    Style = "blue"
	StyleLight   Style = "light"
	StyleDark    Style = "dark"
	StyleAuto    Style = "auto"
)

// ParseStyle accepts only the styles above.
func ParseStyle(value string) (Style, bool) {
	switch style := Style(value); style {
	case StyleDefault, StyleBlue, StyleLight, StyleDark, StyleAuto:
		return style, true
	}
	return StyleDefault, false
}

// Variant is a concrete icon: the shipped colours or one derived from them.
type Variant int

const (
	VariantColour Variant = iota
	VariantBlue
	VariantLight
	VariantDark
)

// Name is a short lowercase label for the variant, fit for a file name.
func (v Variant) Name() string {
	switch v {
	case VariantBlue:
		return "blue"
	case VariantLight:
		return "light"
	case VariantDark:
		return "dark"
	}
	return "default"
}

// Resolve picks the icon to draw. Auto needs the system appearance; until it
// is known the shipped icon stays, so startup never flashes the wrong grey.
func (s Style) Resolve(systemDark, systemKnown bool) Variant {
	switch s {
	case StyleBlue:
		return VariantBlue
	case StyleLight:
		return VariantLight
	case StyleDark:
		return VariantDark
	case StyleAuto:
		if !systemKnown {
			return VariantColour
		}
		if systemDark {
			return VariantDark
		}
		return VariantLight
	}
	return VariantColour
}

// A tone maps the artwork's luminance onto a colour ramp. The artwork sits in
// the upper half of the range, so black/white are stretched to cover it.
type tone struct {
	gamma float64
	stops []stop // ascending by position, first at 0 and last at 1
}

type stop struct {
	position float64
	rgb      [3]float64
}

const (
	artworkBlack = 0.45
	artworkWhite = 0.97
)

// The greys carry a slight cool cast so they do not look flat. Blue runs from
// a deep blue in the folds through a clear mid blue to an almost white sheet.
var tones = map[Variant]tone{
	VariantLight: {gamma: 0.90, stops: []stop{{0, [3]float64{0.495, 0.500, 0.505}}, {1, [3]float64{0.99, 1.00, 1.00}}}},
	VariantDark:  {gamma: 1.25, stops: []stop{{0, [3]float64{0.097, 0.099, 0.103}}, {1, [3]float64{0.776, 0.792, 0.824}}}},
	VariantBlue: {gamma: 1.00, stops: []stop{
		{0, [3]float64{0.05, 0.22, 0.62}},
		{0.55, [3]float64{0.16, 0.50, 0.94}},
		{0.82, [3]float64{0.42, 0.74, 1.00}},
		{1, [3]float64{0.93, 0.97, 1.00}},
	}},
}

// Render returns the PNG for a variant of the colour icon. VariantColour
// returns the source unchanged.
func Render(source []byte, variant Variant) ([]byte, error) {
	tone, ok := tones[variant]
	if !ok {
		return source, nil
	}
	decoded, err := png.Decode(bytes.NewReader(source))
	if err != nil {
		return nil, fmt.Errorf("appicon: decode: %w", err)
	}
	bounds := decoded.Bounds()
	out := image.NewNRGBA(bounds)
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			pixel := color.NRGBAModel.Convert(decoded.At(x, y)).(color.NRGBA)
			if pixel.A == 0 {
				continue
			}
			out.SetNRGBA(x, y, tone.apply(pixel))
		}
	}
	var encoded bytes.Buffer
	if err := png.Encode(&encoded, out); err != nil {
		return nil, fmt.Errorf("appicon: encode: %w", err)
	}
	return encoded.Bytes(), nil
}

func (t tone) apply(pixel color.NRGBA) color.NRGBA {
	luminance := (0.2126*float64(pixel.R) + 0.7152*float64(pixel.G) + 0.0722*float64(pixel.B)) / 255
	position := (luminance - artworkBlack) / (artworkWhite - artworkBlack)
	position = math.Pow(math.Min(1, math.Max(0, position)), t.gamma)
	rgb := t.at(position)
	channel := func(value float64) uint8 {
		return uint8(math.Round(255 * math.Min(1, math.Max(0, value))))
	}
	return color.NRGBA{R: channel(rgb[0]), G: channel(rgb[1]), B: channel(rgb[2]), A: pixel.A}
}

// at interpolates the ramp at a position in [0, 1].
func (t tone) at(position float64) [3]float64 {
	for i := 1; i < len(t.stops); i++ {
		from, to := t.stops[i-1], t.stops[i]
		if position > to.position {
			continue
		}
		mix := (position - from.position) / (to.position - from.position)
		var rgb [3]float64
		for c := range rgb {
			rgb[c] = from.rgb[c] + (to.rgb[c]-from.rgb[c])*mix
		}
		return rgb
	}
	return t.stops[len(t.stops)-1].rgb
}
