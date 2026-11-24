package main

import (
	"bytes"
	"image"
	"image/color"
	"image/png"
	"math"
	"strconv"
	"sync"
)

// Routed through a package var so tests can see which label was set.
var taskbarSetBadgeLabel = platformSetTaskbarBadgeLabel

// Windows taskbar overlay icons are 16×16 logical px, so counts above 99
// collapse to "99+" to stay legible.
const taskbarBadgeMax = 99

var taskbarBadge struct {
	mu    sync.Mutex
	label string
	set   bool
}

// taskbarBadgeLabel turns the frontend's inbox unread count and preference into
// the taskbar overlay label: "" (no overlay) when disabled or zero. ok is false
// when enabled and count isn't a number.
func taskbarBadgeLabel(count any, enabled bool) (label string, ok bool) {
	if !enabled {
		return "", true
	}
	var n float64
	switch value := count.(type) {
	case float64:
		n = value
	case int:
		n = float64(value)
	default:
		return "", false
	}
	if !(n >= 1) { // also rejects NaN
		return "", true
	}
	if n > taskbarBadgeMax {
		return strconv.Itoa(taskbarBadgeMax) + "+", true
	}
	return strconv.Itoa(int(n)), true
}

// setTaskbarBadge shows count on the Windows taskbar button (a no-op
// elsewhere). Like setDockBadge, repeated identical labels are deduplicated.
func setTaskbarBadge(count any, enabled bool) {
	label, ok := taskbarBadgeLabel(count, enabled)
	if !ok {
		return
	}
	taskbarBadge.mu.Lock()
	if taskbarBadge.set && taskbarBadge.label == label {
		taskbarBadge.mu.Unlock()
		return
	}
	taskbarBadge.label, taskbarBadge.set = label, true
	taskbarBadge.mu.Unlock()
	taskbarSetBadgeLabel(label)
}

// refreshTaskbarBadge re-applies the current badge label. Windows clears a
// window's taskbar overlay when the window is hidden to the tray and recreated
// on show, or if the first unread count arrived before the window was visible.
func refreshTaskbarBadge() {
	taskbarBadge.mu.Lock()
	set, label := taskbarBadge.set, taskbarBadge.label
	taskbarBadge.mu.Unlock()
	if set && label != "" {
		taskbarSetBadgeLabel(label)
	}
}

// 5×7 bitmap glyphs for 1- and 2-digit counts.
var badgeGlyphs5x7 = map[rune][7]uint8{
	'0': {0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E},
	'1': {0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E},
	'2': {0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F},
	'3': {0x0E, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0E},
	'4': {0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02},
	'5': {0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E},
	'6': {0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E},
	'7': {0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08},
	'8': {0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E},
	'9': {0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C},
	'+': {0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00},
}

// 4×6 compact glyphs for 3-character labels ("99+").
var badgeGlyphs4x6 = map[rune][6]uint8{
	'9': {0x6, 0x9, 0x9, 0x7, 0x1, 0x6},
	'+': {0x0, 0x4, 0xE, 0x4, 0x0, 0x0},
}

// renderTaskbarBadgePNG draws a 32×32 circular unread badge with crisp white
// digits suitable for Windows' 16×16 (100% DPI) to 32×32 (200% DPI) overlay.
func renderTaskbarBadgePNG(label string) ([]byte, error) {
	const size = 32
	img := image.NewNRGBA(image.Rect(0, 0, size, size))

	// 4×4 supersampled anti-aliased red circle (radius 15 centered at 15.5, 15.5).
	const (
		cx, cy = 15.5, 15.5
		r      = 15.0
		r2     = r * r
	)
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			hits := 0
			for sy := 0; sy < 4; sy++ {
				dy := (float64(y) + (float64(sy)+0.5)/4.0) - cy
				for sx := 0; sx < 4; sx++ {
					dx := (float64(x) + (float64(sx)+0.5)/4.0) - cx
					if dx*dx+dy*dy <= r2 {
						hits++
					}
				}
			}
			if hits > 0 {
				alpha := uint8(math.Round(255.0 * float64(hits) / 16.0))
				img.SetNRGBA(x, y, color.NRGBA{R: 0xE5, G: 0x39, B: 0x35, A: alpha})
			}
		}
	}

	white := color.NRGBA{R: 0xFF, G: 0xFF, B: 0xFF, A: 0xFF}
	runes := []rune(label)

	switch len(runes) {
	case 1:
		drawGlyphs5x7(img, runes, 3, 0, white)
	case 2:
		drawGlyphs5x7(img, runes, 2, 2, white)
	default:
		// Compact 4×6 at scale 2: "99+" is 8+1+8+1+6 = 24px wide × 12px tall.
		drawCompact99Plus(img, runes, white)
	}

	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

func drawGlyphs5x7(img *image.NRGBA, runes []rune, scale, gap int, fg color.NRGBA) {
	glyphW := 5 * scale
	glyphH := 7 * scale
	totalW := len(runes)*glyphW + (len(runes)-1)*gap
	startX := (img.Bounds().Dx() - totalW) / 2
	startY := (img.Bounds().Dy() - glyphH) / 2

	for i, ch := range runes {
		rows, ok := badgeGlyphs5x7[ch]
		if !ok {
			continue
		}
		ox := startX + i*(glyphW+gap)
		for rowIdx, bits := range rows {
			for col := 0; col < 5; col++ {
				if bits&(1<<(4-col)) == 0 {
					continue
				}
				fillRect(img, ox+col*scale, startY+rowIdx*scale, scale, scale, fg)
			}
		}
	}
}

func drawCompact99Plus(img *image.NRGBA, runes []rune, fg color.NRGBA) {
	const (
		scale  = 2
		glyphW = 4 * scale
		glyphH = 6 * scale
		gap    = 1
	)
	totalW := len(runes)*glyphW + (len(runes)-1)*gap
	startX := (img.Bounds().Dx() - totalW) / 2
	startY := (img.Bounds().Dy() - glyphH) / 2

	for i, ch := range runes {
		rows, ok := badgeGlyphs4x6[ch]
		if !ok {
			continue
		}
		ox := startX + i*(glyphW+gap)
		for rowIdx, bits := range rows {
			for col := 0; col < 4; col++ {
				if bits&(1<<(3-col)) == 0 {
					continue
				}
				fillRect(img, ox+col*scale, startY+rowIdx*scale, scale, scale, fg)
			}
		}
	}
}

func fillRect(img *image.NRGBA, x, y, w, h int, c color.NRGBA) {
	b := img.Bounds()
	for dy := 0; dy < h; dy++ {
		py := y + dy
		if py < b.Min.Y || py >= b.Max.Y {
			continue
		}
		for dx := 0; dx < w; dx++ {
			px := x + dx
			if px < b.Min.X || px >= b.Max.X {
				continue
			}
			img.SetNRGBA(px, py, c)
		}
	}
}
