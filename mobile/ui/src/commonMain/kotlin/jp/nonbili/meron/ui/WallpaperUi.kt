package jp.nonbili.meron.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Image
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.ui.resources.Res
import jp.nonbili.meron.ui.resources.wp_aurora
import jp.nonbili.meron.ui.resources.wp_breeze
import jp.nonbili.meron.ui.resources.wp_desert
import jp.nonbili.meron.ui.resources.wp_forest
import jp.nonbili.meron.ui.resources.wp_galaxy
import jp.nonbili.meron.ui.resources.wp_marble
import jp.nonbili.meron.ui.resources.wp_mountain
import jp.nonbili.meron.ui.resources.wp_nebula
import jp.nonbili.meron.ui.resources.wp_ocean
import jp.nonbili.meron.ui.resources.wp_raindrops
import jp.nonbili.meron.ui.resources.wp_sakura
import jp.nonbili.meron.ui.resources.wp_shapes
import jp.nonbili.meron.ui.resources.wp_sunset
import jp.nonbili.meron.ui.resources.wp_vintage
import kotlinx.coroutines.withContext
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.painterResource

// Mirrors frontend/src/lib/wallpapers.ts WALLPAPER_PRESETS. Photographic
// presets resolve to bundled drawables (see wallpaperImageRes); the rest are
// drawn as approximations.
private val wallpaperPresets =
    listOf(
        "plain" to "Plain",
        "doodle" to "Doodle",
        "dots" to "Linear Dots",
        "grid" to "Classic Grid",
        "stripes" to "Diagonal Stripes",
        "hexagon" to "Hexagon Grid",
        "isometric" to "Isometric Cubes",
        "waves" to "Flowing Waves",
        "nordic" to "Nordic Pattern",
        "topography" to "Topography",
        "constellation" to "Constellation",
        "aurora" to "Aurora",
        "nebula" to "Nebula",
        "sunset" to "Sunset Glow",
        "forest" to "Forest Mist",
        "desert" to "Desert Dunes",
        "ocean" to "Tranquil Ocean",
        "mountain" to "Mountain Range",
        "breeze" to "Soft Breeze",
        "galaxy" to "Spiral Galaxy",
        "shapes" to "Abstract Shapes",
        "sakura" to "Sakura Watercolor",
        "vintage" to "Vintage Parchment",
        "raindrops" to "Raindrops",
        "marble" to "Sleek Marble",
        "cyberpunk" to "Cyberpunk Grid",
        "matrix" to "Digital Matrix",
        "autumn" to "Autumn Leaves",
        "nightsky" to "Celestial Night",
    )

// Photographic presets ship as bundled drawables so the preview matches desktop.
private fun wallpaperImageRes(presetId: String): DrawableResource? =
    when (presetId) {
        "aurora" -> Res.drawable.wp_aurora
        "nebula" -> Res.drawable.wp_nebula
        "sunset" -> Res.drawable.wp_sunset
        "forest" -> Res.drawable.wp_forest
        "desert" -> Res.drawable.wp_desert
        "ocean" -> Res.drawable.wp_ocean
        "mountain" -> Res.drawable.wp_mountain
        "breeze" -> Res.drawable.wp_breeze
        "galaxy" -> Res.drawable.wp_galaxy
        "shapes" -> Res.drawable.wp_shapes
        "sakura" -> Res.drawable.wp_sakura
        "vintage" -> Res.drawable.wp_vintage
        "raindrops" -> Res.drawable.wp_raindrops
        "marble" -> Res.drawable.wp_marble
        else -> null
    }

private fun normalizedWallpaperPresetId(presetId: String): String = presetId.ifBlank { "plain" }

internal fun wallpaperPresetDisplayName(presetId: String): String = wallpaperPresets.firstOrNull { it.first == normalizedWallpaperPresetId(presetId) }?.second ?: "Plain"

@Composable
internal fun WallpaperPickerPage(
    selected: String,
    previewPresetId: String,
    previewCustomUrl: String,
    onSelect: (String) -> Unit,
    onUpload: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val normalizedSelected = if (selected == "__custom") selected else normalizedWallpaperPresetId(selected)
    val normalizedPreviewPresetId = normalizedWallpaperPresetId(previewPresetId)
    var localSelected by remember(selected, previewPresetId, previewCustomUrl) { mutableStateOf(normalizedSelected) }
    var localPresetId by remember(selected, previewPresetId, previewCustomUrl) { mutableStateOf(normalizedPreviewPresetId) }
    var localCustomUrl by remember(selected, previewPresetId, previewCustomUrl) { mutableStateOf(previewCustomUrl) }
    LazyColumn(modifier) {
        item {
            ChatWallpaperPreview(
                presetId = localPresetId,
                customUrl = localCustomUrl,
            )
        }
        item {
            WallpaperPresetGrid(
                selected = localSelected,
                onSelect = {
                    localSelected = it
                    localPresetId = it
                    localCustomUrl = ""
                    onSelect(it)
                },
                onUpload = onUpload,
                uploadLabel = tr("wallpaper.uploadCustom"),
            )
        }
        item { Spacer(Modifier.height(24.dp)) }
    }
}

@Composable
internal fun WallpaperRowPreview(
    presetId: String,
    customUrl: String,
) {
    Box(
        modifier =
            Modifier
                .size(width = 56.dp, height = 40.dp)
                .clip(RoundedCornerShape(10.dp))
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(10.dp)),
    ) {
        ChatWallpaperBackground(
            presetId = presetId,
            customUrl = customUrl,
            modifier = Modifier.matchParentSize(),
            fallback = MaterialTheme.colorScheme.surface,
        )
    }
}

@Composable
internal fun WallpaperPresetGrid(
    selected: String,
    onSelect: (String) -> Unit,
    onUpload: () -> Unit,
    uploadLabel: String,
) {
    val rows =
        remember {
            (listOf<Pair<String, String>?>(null) + wallpaperPresets.map { it.first to it.second }).chunked(3)
        }
    Column(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        rows.forEach { row ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                row.forEach { preset ->
                    if (preset == null) {
                        WallpaperUploadTile(
                            label = uploadLabel,
                            onClick = onUpload,
                            modifier = Modifier.weight(1f),
                        )
                    } else {
                        val (id, label) = preset
                        WallpaperPresetTile(
                            presetId = id,
                            label = label,
                            selected = selected == id,
                            onClick = { onSelect(id) },
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
                repeat(3 - row.size) {
                    Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun WallpaperUploadTile(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(14.dp)
    Box(
        modifier =
            modifier
                .aspectRatio(1.58f)
                .clip(shape)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, shape)
                .clickable(onClick = onClick)
                .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Icon(
                Icons.Filled.Image,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(20.dp),
            )
            Text(
                label,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun WallpaperPresetTile(
    presetId: String,
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(14.dp)
    val borderColor = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant
    Box(
        modifier =
            modifier
                .aspectRatio(1.58f)
                .clip(shape)
                .border(if (selected) 2.dp else 1.dp, borderColor, shape)
                .clickable(onClick = onClick)
                .semantics { contentDescription = label },
    ) {
        ChatWallpaperBackground(
            presetId = presetId,
            customUrl = "",
            modifier = Modifier.matchParentSize(),
            fallback = MaterialTheme.colorScheme.surface,
        )
        if (selected) {
            Box(
                modifier =
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(6.dp)
                        .size(22.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Filled.Check,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(15.dp),
                )
            }
        }
    }
}

// Paints a chat wallpaper as a fill layer: a bundled photographic drawable, a
// loaded custom image, or a drawn pattern/gradient approximation. Place it as
// the first child of a Box with the content drawn on top. Shared by the
// settings preview and the conversation thread background.
@Composable
internal fun ChatWallpaperBackground(
    presetId: String,
    customUrl: String,
    modifier: Modifier = Modifier,
    fallback: Color = MaterialTheme.colorScheme.background,
) {
    val dark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    var customImage by remember(customUrl) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(customUrl) {
        customImage = null
        if (customUrl.isNotBlank()) {
            customImage = withContext(ioDispatcher) { loadImageBitmapRef(customUrl) }
        }
    }

    val imageRes = if (customImage == null) wallpaperImageRes(presetId) else null
    Box(
        modifier =
            modifier.then(
                if (customImage == null && imageRes == null) {
                    Modifier.chatWallpaper(presetId, dark, fallback)
                } else {
                    Modifier
                },
            ),
    ) {
        val bmp = customImage
        if (bmp != null) {
            Image(
                bitmap = bmp,
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        } else if (imageRes != null) {
            Image(
                painter = painterResource(imageRes),
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        }
    }
}

// Telegram-style preview: paints the selected wallpaper with a couple of sample
// chat bubbles on top, so the choice reads at a glance before opening a thread.
@Composable
internal fun ChatWallpaperPreview(
    presetId: String,
    customUrl: String,
    modifier: Modifier = Modifier,
) {
    val chat = LocalChatColors.current
    Box(
        modifier =
            modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 4.dp)
                .height(160.dp)
                .clip(RoundedCornerShape(16.dp)),
    ) {
        ChatWallpaperBackground(
            presetId = presetId,
            customUrl = customUrl,
            modifier = Modifier.matchParentSize(),
            fallback = MaterialTheme.colorScheme.surface,
        )
        Column(
            modifier = Modifier.fillMaxSize().padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            PreviewBubble("Welcome to the conversation", chat.bubbleIn, chat.bubbleInText, false)
            PreviewBubble("This is your chat background", chat.bubbleOut, chat.bubbleOutText, true)
        }
    }
}

@Composable
private fun PreviewBubble(
    text: String,
    color: Color,
    textColor: Color,
    outgoing: Boolean,
) {
    val shape =
        if (outgoing) {
            RoundedCornerShape(topStart = 14.dp, topEnd = 4.dp, bottomEnd = 14.dp, bottomStart = 14.dp)
        } else {
            RoundedCornerShape(topStart = 4.dp, topEnd = 14.dp, bottomEnd = 14.dp, bottomStart = 14.dp)
        }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = if (outgoing) Arrangement.End else Arrangement.Start,
    ) {
        Text(
            text,
            color = textColor,
            style = MaterialTheme.typography.bodySmall,
            modifier =
                Modifier
                    .widthIn(max = 220.dp)
                    .clip(shape)
                    .background(color)
                    .padding(horizontal = 12.dp, vertical = 7.dp),
        )
    }
}

// Approximates the desktop wallpaper presets: a base gradient plus an optional
// grid/dot pattern overlay drawn on top.
private fun Modifier.chatWallpaper(
    presetId: String,
    dark: Boolean,
    fallback: Color,
): Modifier {
    val colors =
        when (presetId) {
            "doodle" -> if (dark) listOf(Color(0xFF11131F), Color(0xFF0C0A16)) else listOf(Color(0xFFEEF2FF), Color(0xFFF5F3FF), Color(0xFFFEF2F2))
            "dots" -> if (dark) listOf(Color(0xFF121212), Color(0xFF171717)) else listOf(Color(0xFFFAFAF9), Color(0xFFF5F5F7))
            "grid" -> if (dark) listOf(Color(0xFF090E1A), Color(0xFF020617)) else listOf(Color(0xFFF3F4F6), Color(0xFFE5E7EB))
            "stripes" -> if (dark) listOf(Color(0xFF04120E), Color(0xFF020706)) else listOf(Color(0xFFF0FDF4), Color(0xFFDCFCE7))
            "hexagon" -> if (dark) listOf(Color(0xFF021613), Color(0xFF010807)) else listOf(Color(0xFFF0FDFA), Color(0xFFCCFBF1))
            "isometric" -> if (dark) listOf(Color(0xFF0B0F17), Color(0xFF05070C)) else listOf(Color(0xFFF1F5F9), Color(0xFFE2E8F0))
            "waves" -> if (dark) listOf(Color(0xFF06101A), Color(0xFF030A12)) else listOf(Color(0xFFF0FDF4), Color(0xFFE0F2FE))
            "nordic" -> if (dark) listOf(Color(0xFF0E0F12), Color(0xFF070809)) else listOf(Color(0xFFF9FAFB), Color(0xFFF3F4F6))
            "topography" -> if (dark) listOf(Color(0xFF0A1018), Color(0xFF05090F)) else listOf(Color(0xFFF0F4F8), Color(0xFFE2E8F0))
            "constellation" -> if (dark) listOf(Color(0xFF070B1A), Color(0xFF03050F)) else listOf(Color(0xFFE0F2FE), Color(0xFFE0E7FF))
            "cyberpunk" -> listOf(Color(0xFF0F0A1C), Color(0xFF07040D))
            "matrix" -> listOf(Color(0xFF02040A), Color(0xFF010204))
            "nightsky" -> listOf(Color(0xFF0B0F19), Color(0xFF050510), Color(0xFF020617))
            "autumn" -> if (dark) listOf(Color(0xFF1C1206), Color(0xFF0E0903)) else listOf(Color(0xFFFFFBEB), Color(0xFFFFEDD5), Color(0xFFFEF3C7))
            else -> listOf(fallback, fallback)
        }
    val base = Brush.linearGradient(colors)
    val accentPattern = if (dark) Color.White.copy(alpha = 0.05f) else Color(0xFF6366F1).copy(alpha = 0.10f)
    return this
        .background(base)
        .drawBehind {
            when (presetId) {
                "grid", "isometric", "nordic" -> {
                    val step = 24.dp.toPx()
                    var x = step
                    while (x < size.width) {
                        drawLine(accentPattern, Offset(x, 0f), Offset(x, size.height), 1f)
                        x += step
                    }
                    var y = step
                    while (y < size.height) {
                        drawLine(accentPattern, Offset(0f, y), Offset(size.width, y), 1f)
                        y += step
                    }
                }

                "dots", "doodle", "topography", "raindrops" -> {
                    val step = 20.dp.toPx()
                    val radius = 1.5.dp.toPx()
                    var y = step / 2
                    while (y < size.height) {
                        var x = step / 2
                        while (x < size.width) {
                            drawCircle(accentPattern, radius, Offset(x, y))
                            x += step
                        }
                        y += step
                    }
                }

                "stripes", "waves", "cyberpunk", "matrix" -> {
                    val patternColor =
                        when (presetId) {
                            "stripes", "waves" -> if (dark) Color(0xFF34D399).copy(alpha = 0.10f) else Color(0xFF10B981).copy(alpha = 0.12f)
                            else -> Color(0xFF22D3EE).copy(alpha = 0.16f)
                        }
                    val step = 22.dp.toPx()
                    var x = -size.height
                    while (x < size.width) {
                        drawLine(patternColor, Offset(x, size.height), Offset(x + size.height, 0f), 1.5f)
                        x += step
                    }
                }

                "constellation", "nightsky", "galaxy" -> {
                    val star = Color.White.copy(alpha = if (dark) 0.5f else 0.35f)
                    val seedPoints =
                        listOf(
                            0.12f to 0.22f,
                            0.3f to 0.55f,
                            0.48f to 0.18f,
                            0.62f to 0.7f,
                            0.78f to 0.32f,
                            0.88f to 0.6f,
                            0.2f to 0.82f,
                            0.55f to 0.4f,
                        )
                    seedPoints.forEach { (fx, fy) ->
                        drawCircle(star, 1.4.dp.toPx(), Offset(fx * size.width, fy * size.height))
                    }
                }
            }
        }
}
