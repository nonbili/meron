"""Check tray SVGs with Qt5, including hosts without SVG mask support.

Requires PyQt5. Run with QT_QPA_PLATFORM=offscreen python this-file.py.
"""

from pathlib import Path

from PyQt5.QtCore import Qt
from PyQt5.QtGui import QGuiApplication, QImage, QPainter
from PyQt5.QtSvg import QSvgRenderer

app = QGuiApplication([])
assets = Path(__file__).resolve().parents[1] / "desktop" / "build"
for name in ("tray-symbolic.svg", "tray-unread-symbolic.svg"):
    renderer = QSvgRenderer(str(assets / name))
    assert renderer.isValid(), name
    image = QImage(128, 128, QImage.Format_ARGB32)
    image.fill(Qt.transparent)
    painter = QPainter(image)
    renderer.render(painter)
    painter.end()
    opaque = sum(
        image.pixelColor(x, y).alpha() > 200
        for x in range(128)
        for y in range(128)
    )
    assert 1000 < opaque < 15000, (name, opaque)
    assert image.pixelColor(0, 0).alpha() == 0, (name, "solid square")
    if "unread" in name:
        badge = image.pixelColor(112, 112)
        assert badge.red() > 240 and badge.green() < 90 and badge.blue() < 90, name
    print(f"{name}: Qt5 render passed")
