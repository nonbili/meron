#!/usr/bin/env python3
"""Generate every single-colour Meron icon from one vector glyph.

The glyph is the app icon's envelope reduced to one colour: its outline and the
bow-tie of paper showing inside, each a polygon with rounded corners. The
numbers below were fitted to desktop/build/appicon.png (1024 px frame), so the
glyph keeps the icon's proportions while staying a clean, symmetric vector.

Writes:
  desktop/build/tray-symbolic.svg, tray-unread-symbolic.svg   Linux tray
  desktop/build/tray-fallback-mono.png, -unread.png           Linux tray fallback
  desktop/build/trayicon-mono.png                             macOS menu bar
  mobile/android/.../drawable/ic_stat_mail.xml                notifications, widgets
  mobile/android/.../drawable/ic_launcher_monochrome.xml      themed launcher icon

Requires Pillow. Run from anywhere: python3 scripts/icons/glyph.py
"""

import math
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "desktop" / "build"
DRAWABLE = ROOT / "mobile" / "android" / "src" / "main" / "res" / "drawable"

# Glyph geometry in the 1024 px frame of desktop/build/appicon.png, symmetric
# about CX. Each polygon corner lists (x, y, fillet radius).
CX = 512.5
OUTER = [
    (82.5, 49, 100),  # left lobe: the top-left corner of the envelope
    (CX, 338, 32),  # the valley between the flaps
    (2 * CX - 82.5, 49, 100),
    (2 * CX - 82.5, 889.75, 121.75),
    (82.5, 889.75, 121.75),
]
HOLE = [
    (226.5, 378.75, 28.5),
    (CX, 578, 50),  # tip of the top flap
    (2 * CX - 226.5, 378.75, 28.5),
    (2 * CX - 226.5, 779, 32),
    (CX, 562, 109.5),  # peak of the bottom flap
    (226.5, 779, 32),
]

# The fitted glyph suits launcher sizes. At tray and notification sizes its
# bands crowd out the paper, so small icons widen the paper by this many units,
# which thins every band (the frame is 144 units at its sides).
SMALL_WEIGHT = 35

BADGE = "#ff3b30"  # unread dot, also used by the macOS badge


def hole(widen=0):
    """The paper polygon, widened by widen units on every side.

    Each edge moves outwards; corners keep their shape, so the convex corners'
    radii grow and the flap tips' shrink by the same amount.
    """
    if not widen:
        return HOLE
    pts = [(x, y) for x, y, _ in HOLE]
    n = len(pts)
    # Clockwise on screen (positive area with y down): outside is on the left.
    area = sum(pts[i][0] * pts[(i + 1) % n][1] - pts[(i + 1) % n][0] * pts[i][1] for i in range(n))
    lines = []
    for i in range(n):
        (x0, y0), (x1, y1) = pts[i], pts[(i + 1) % n]
        dx, dy = x1 - x0, y1 - y0
        length = math.hypot(dx, dy)
        nx, ny = (dy / length, -dx / length) if area > 0 else (-dy / length, dx / length)
        lines.append(((x0 + nx * widen, y0 + ny * widen), (dx, dy)))
    out = []
    for i in range(n):
        (p, u), (q, v) = lines[i - 1], lines[i]
        t = ((q[0] - p[0]) * v[1] - (q[1] - p[1]) * v[0]) / (u[0] * v[1] - u[1] * v[0])
        radius = HOLE[i][2]
        tip = i in (1, 4)  # the flap tips point into the paper
        out.append((p[0] + u[0] * t, p[1] + u[1] * t, max(4, radius - widen) if tip else radius + widen))
    return out


class Corner:
    """A polygon corner rounded by a circular arc from start to end."""

    def __init__(self, prev, at, nxt, radius):
        ax, ay = prev[0] - at[0], prev[1] - at[1]
        bx, by = nxt[0] - at[0], nxt[1] - at[1]
        la, lb = math.hypot(ax, ay), math.hypot(bx, by)
        ax, ay, bx, by = ax / la, ay / la, bx / lb, by / lb
        angle = math.acos(max(-1.0, min(1.0, ax * bx + ay * by)))
        tangent = radius / math.tan(angle / 2)
        self.radius = radius
        self.start = (at[0] + ax * tangent, at[1] + ay * tangent)
        self.end = (at[0] + bx * tangent, at[1] + by * tangent)
        mx, my = ax + bx, ay + by
        ml = math.hypot(mx, my)
        distance = radius / math.sin(angle / 2)
        self.centre = (at[0] + mx / ml * distance, at[1] + my / ml * distance)
        a0 = math.atan2(self.start[1] - self.centre[1], self.start[0] - self.centre[0])
        a1 = math.atan2(self.end[1] - self.centre[1], self.end[0] - self.centre[0])
        sweep = (a1 - a0 + math.pi) % (2 * math.pi) - math.pi
        self.a0, self.sweep = a0, sweep

    def points(self, steps=48):
        cx, cy = self.centre
        return [
            (cx + self.radius * math.cos(self.a0 + self.sweep * k / steps),
             cy + self.radius * math.sin(self.a0 + self.sweep * k / steps))
            for k in range(steps + 1)
        ]


def corners(polygon):
    n = len(polygon)
    return [Corner(polygon[i - 1][:2], polygon[i][:2], polygon[(i + 1) % n][:2], polygon[i][2]) for i in range(n)]


def num(value):
    text = "%.2f" % value
    return text.rstrip("0").rstrip(".") if "." in text else text


def pt(p):
    return "%s,%s" % (num(p[0]), num(p[1]))


def contour_path(cs):
    out = "M" + pt(cs[0].start)
    for i, c in enumerate(cs):
        if i:
            out += "L" + pt(c.start)
        out += "A%s,%s 0 0,%d %s" % (num(c.radius), num(c.radius), 1 if c.sweep > 0 else 0, pt(c.end))
    return out + "Z"


def contour_points(cs):
    pts = []
    for c in cs:
        pts += c.points()
    return pts


class Placement:
    """Maps the glyph's 1024 px frame into a target box, keeping its aspect."""

    def __init__(self, left, top, right, bottom):
        xs = [p[0] for p in contour_points(corners(OUTER))]
        ys = [p[1] for p in contour_points(corners(OUTER))]
        gx0, gx1, gy0, gy1 = min(xs), max(xs), min(ys), max(ys)
        self.scale = min((right - left) / (gx1 - gx0), (bottom - top) / (gy1 - gy0))
        self.dx = (left + right) / 2 - self.scale * (gx0 + gx1) / 2
        self.dy = (top + bottom) / 2 - self.scale * (gy0 + gy1) / 2

    def to_target(self, p):
        return (self.dx + self.scale * p[0], self.dy + self.scale * p[1])

    def to_glyph(self, p):
        return ((p[0] - self.dx) / self.scale, (p[1] - self.dy) / self.scale)


def inside(point, polygon):
    """Even-odd point-in-polygon test."""
    x, y = point
    result = False
    for (x0, y0), (x1, y1) in zip(polygon, polygon[1:] + polygon[:1]):
        if (y0 > y) != (y1 > y) and x < x0 + (y - y0) * (x1 - x0) / (y1 - y0):
            result = not result
    return result


def line_circle(p0, p1, centre, r):
    """Parameters t in [0, 1] where the segment p0-p1 crosses the circle."""
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]
    fx, fy = p0[0] - centre[0], p0[1] - centre[1]
    a, b, c = dx * dx + dy * dy, 2 * (fx * dx + fy * dy), fx * fx + fy * fy - r * r
    disc = b * b - 4 * a * c
    if disc < 0:
        return []
    roots = [(-b - math.sqrt(disc)) / (2 * a), (-b + math.sqrt(disc)) / (2 * a)]
    return [t for t in roots if 0 <= t <= 1]


def lerp(p0, p1, t):
    return (p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t)


def gap_arc(start, end, centre, r, hole_points):
    """The arc of the badge gap from start to end that runs through the glyph."""
    a0 = math.atan2(start[1] - centre[1], start[0] - centre[0])
    a1 = math.atan2(end[1] - centre[1], end[0] - centre[0])
    for sweep in ((a1 - a0) % (2 * math.pi), (a1 - a0) % (2 * math.pi) - 2 * math.pi):
        mid = a0 + sweep / 2
        probe = (centre[0] + r * math.cos(mid), centre[1] + r * math.sin(mid))
        if inside(probe, OUTER_POINTS) and not inside(probe, hole_points):
            return "A%s,%s 0 %d,%d %s" % (num(r), num(r), abs(sweep) > math.pi, sweep > 0, pt(end))
    raise AssertionError("badge gap does not cross the glyph")


def segments(cs):
    """A rounded polygon as its alternating arcs and straight edges, in order."""
    out = []
    for i, c in enumerate(cs):
        out.append(("arc", c))
        out.append(("line", c.end, cs[(i + 1) % len(cs)].start))
    return out


def seg_point(seg, t):
    if seg[0] == "line":
        return lerp(seg[1], seg[2], t)
    c = seg[1]
    angle = c.a0 + c.sweep * t
    return (c.centre[0] + c.radius * math.cos(angle), c.centre[1] + c.radius * math.sin(angle))


def seg_crossings(seg, centre, r):
    """Parameters t in [0, 1] where a segment crosses the gap circle."""
    if seg[0] == "line":
        return line_circle(seg[1], seg[2], centre, r)
    c = seg[1]
    dx, dy = centre[0] - c.centre[0], centre[1] - c.centre[1]
    d = math.hypot(dx, dy)
    if d == 0 or d > c.radius + r or d < abs(c.radius - r):
        return []
    along = (c.radius ** 2 - r ** 2 + d * d) / (2 * d)
    across = math.sqrt(max(0.0, c.radius ** 2 - along ** 2))
    ts = []
    for sign in (1, -1):
        x = c.centre[0] + (along * dx - sign * across * dy) / d
        y = c.centre[1] + (along * dy + sign * across * dx) / d
        theta = math.atan2(y - c.centre[1], x - c.centre[0])
        turn = (theta - c.a0) % (2 * math.pi) if c.sweep > 0 else -((c.a0 - theta) % (2 * math.pi))
        t = turn / c.sweep
        if 0 <= t <= 1:
            ts.append(t)
    return ts


def crossings(segs, centre, r):
    """Where a closed contour enters and leaves the gap circle, as (index, t)."""
    found = []
    for i, seg in enumerate(segs):
        for t in seg_crossings(seg, centre, r):
            q = seg_point(seg, t)
            if all(math.hypot(q[0] - f[2][0], q[1] - f[2][1]) > 1e-6 for f in found):
                found.append((i, t, q))
    if not found:
        return None
    assert len(found) == 2, "the gap crosses an outline more than twice"

    def enters(f):
        i, t, _ = f
        ahead = seg_point(segs[i], min(1.0, t + 1e-4)) if t < 1 else seg_point(segs[(i + 1) % len(segs)], 1e-4)
        return math.hypot(ahead[0] - centre[0], ahead[1] - centre[1]) < r

    first, second = found
    return (first, second) if enters(first) else (second, first)


def seg_command(seg, t, reverse=False):
    """Drawing command along seg to parameter t (backwards when reverse)."""
    end = pt(seg_point(seg, t))
    if seg[0] == "line":
        return "L" + end
    c = seg[1]
    return "A%s,%s 0 0,%d %s" % (num(c.radius), num(c.radius), (c.sweep > 0) != reverse, end)


def walk(segs, frm, to, reverse=False):
    """Commands along a closed contour from position frm to position to."""
    (i, ta), (j, tb) = frm, to
    n = len(segs)
    out = ""
    if not reverse:
        if i == j and ta <= tb:
            return seg_command(segs[i], tb)
        out += seg_command(segs[i], 1)
        k = (i + 1) % n
        while k != j:
            out += seg_command(segs[k], 1)
            k = (k + 1) % n
        return out + seg_command(segs[j], tb)
    if i == j and tb <= ta:
        return seg_command(segs[i], tb, True)
    out += seg_command(segs[i], 0, True)
    k = (i - 1) % n
    while k != j:
        out += seg_command(segs[k], 0, True)
        k = (k - 1) % n
    return out + seg_command(segs[j], tb, True)


def unread_path(centre, r, paper):
    """The glyph with a circular gap around the unread badge.

    The outline is followed outside the gap and joined around it. If the gap
    also reaches the paper, the two outlines become one: across the frame
    along the gap, back around the paper the other way, and out along the gap
    again.
    """
    outer, inner = segments(corners(OUTER)), segments(corners(paper))
    hole_points = contour_points(corners(paper))
    cross_outer = crossings(outer, centre, r)
    assert cross_outer, "the gap misses the outline"
    (oi, ot, o_in), (oj, ou, o_out) = cross_outer
    path = "M" + pt(o_out) + walk(outer, (oj, ou), (oi, ot))
    cross_paper = crossings(inner, centre, r)
    if cross_paper is None:
        path += gap_arc(o_in, o_out, centre, r, hole_points) + "Z"
        return path + contour_path(corners(paper))
    (hi, ht, h_in), (hj, hu, h_out) = cross_paper
    path += gap_arc(o_in, h_in, centre, r, hole_points)
    path += walk(inner, (hi, ht), (hj, hu), reverse=True)
    return path + gap_arc(h_out, o_out, centre, r, hole_points) + "Z"


OUTER_POINTS = contour_points(corners(OUTER))


def glyph_path(paper):
    return contour_path(corners(OUTER)) + contour_path(corners(paper))

# The tray glyph fills a 16 px canvas between these edges; the unread badge sits
# in the bottom-right corner with a gap around it.
TRAY_BOX = (0.25, 1.125, 15.75, 14.875)
TRAY_BADGE = ((14, 14), 2.0)
TRAY_GAP = 2.75


def tray_svg(unread, paper):
    place = Placement(*TRAY_BOX)
    if unread:
        d = unread_path(place.to_glyph(TRAY_BADGE[0]), TRAY_GAP / place.scale, paper)
    else:
        d = glyph_path(paper)
    badge = (
        '\n  <circle class="error" cx="%s" cy="%s" r="%s" fill="%s"/>'
        % ("%g" % TRAY_BADGE[0][0], "%g" % TRAY_BADGE[0][1], "%g" % TRAY_BADGE[1], BADGE)
        if unread else ""
    )
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">\n'
        '  <defs><style id="current-color-scheme" type="text/css">.ColorScheme-Text { color:#232629; }</style></defs>\n'
        '  <path class="ColorScheme-Text" fill="currentColor" fill-rule="evenodd" '
        'transform="translate(%s %s) scale(%s)" d="%s"/>%s\n'
        "</svg>\n" % ("%.4f" % place.dx, "%.4f" % place.dy, "%.6f" % place.scale, d, badge)
    )


def raster(size, box, colour, paper, unread=False, supersample=8):
    """Rasterise the glyph at size px with the 16 px tray layout scaled up."""
    big = size * supersample
    k = big / 16
    place = Placement(*(v * k for v in box))
    mask = Image.new("L", (big, big), 0)
    draw = ImageDraw.Draw(mask)
    draw.polygon([place.to_target(p) for p in OUTER_POINTS], fill=255)
    draw.polygon([place.to_target(p) for p in contour_points(corners(paper))], fill=0)
    (bx, by), br = TRAY_BADGE
    if unread:
        g = TRAY_GAP
        draw.ellipse(((bx - g) * k, (by - g) * k, (bx + g) * k, (by + g) * k), fill=0)
    mask = mask.resize((size, size), Image.LANCZOS)
    out = Image.new("RGBA", (size, size), colour + (0,))
    out.putalpha(mask)
    if unread:
        dot = Image.new("L", (big, big), 0)
        ImageDraw.Draw(dot).ellipse(((bx - br) * k, (by - br) * k, (bx + br) * k, (by + br) * k), fill=255)
        layer = Image.new("RGBA", (size, size), tuple(int(BADGE[i:i + 2], 16) for i in (1, 3, 5)) + (0,))
        layer.putalpha(dot.resize((size, size), Image.LANCZOS))
        out.alpha_composite(layer)
    return out


def android_vector(comment, size_dp, viewport, box, colour, paper):
    place = Placement(*box)
    return (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        "<!-- %s\n"
        "     Generated by scripts/icons/glyph.py. -->\n"
        '<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
        '    android:width="%ddp"\n'
        '    android:height="%ddp"\n'
        '    android:viewportWidth="%d"\n'
        '    android:viewportHeight="%d">\n'
        "    <group\n"
        '        android:scaleX="%.6f"\n'
        '        android:scaleY="%.6f"\n'
        '        android:translateX="%.4f"\n'
        '        android:translateY="%.4f">\n'
        "        <path\n"
        '            android:fillColor="%s"\n'
        '            android:fillType="evenOdd"\n'
        '            android:pathData="%s" />\n'
        "    </group>\n"
        "</vector>\n"
        % (comment, size_dp, size_dp, viewport, viewport, place.scale, place.scale, place.dx, place.dy, colour, glyph_path(paper))
    )


def main():
    small = hole(SMALL_WEIGHT)
    (BUILD / "tray-symbolic.svg").write_text(tray_svg(False, small))
    (BUILD / "tray-unread-symbolic.svg").write_text(tray_svg(True, small))
    # Hosts without symbolic icons cannot recolour the fallback, so it is a
    # neutral grey that shows on light and dark panels alike.
    raster(128, TRAY_BOX, (128, 128, 128), small).save(BUILD / "tray-fallback-mono.png", optimize=True)
    raster(128, TRAY_BOX, (128, 128, 128), small, unread=True).save(BUILD / "tray-fallback-mono-unread.png", optimize=True)
    # macOS template image: black, and as wide as the canvas.
    raster(64, (0, 0.5, 16, 15.5), (0, 0, 0), small).save(BUILD / "trayicon-mono.png", optimize=True)
    (DRAWABLE / "ic_stat_mail.xml").write_text(android_vector(
        "Status-bar small icon: must stay alpha-only white; Android applies the tint.\n"
        "     The glyph fills Material's 20dp live area.",
        24, 24, (2, 2, 22, 22), "@android:color/white", small))
    # Over the envelope in ic_launcher_foreground, so a themed icon keeps the
    # shipped icon's size and place.
    (DRAWABLE / "ic_launcher_monochrome.xml").write_text(android_vector(
        "Themed launcher icon (Android 13+): the system tints this shape and puts it on\n"
        "     a plate that follows light and dark.",
        108, 108, (30.25, 33.25, 77.5, 74.75), "#FFFFFFFF", HOLE))


if __name__ == "__main__":
    main()
