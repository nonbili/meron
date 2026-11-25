use super::*;

/// Rewrite `cid:<id>` references (e.g. `src="cid:logo"`) to the `/media/<key>` path
/// the matching inline image was written to, so HTML mode can load them.
pub(super) fn rewrite_cid_refs(html: &str, cid_keys: &[(String, String)]) -> String {
    let mut out = html.to_string();
    for (cid, key) in cid_keys {
        if cid.is_empty() {
            continue;
        }
        out = out.replace(&format!("cid:{cid}"), &format!("/media/{key}"));
    }
    out
}

/// Strip script-bearing markup from email HTML while preserving layout/styling.
/// This is the independent second boundary behind the iframe's `default-src 'none'`
/// CSP: even if the CSP fails to apply, the markup itself carries no `<script>`,
/// no `on*` handlers, no `javascript:`/`vbscript:` URLs, and no embedding tags
/// (`<iframe>`/`<object>`/`<embed>`/`<form>`/`<meta>`/`<base>`/`<link>`), so the
/// `allow-same-origin allow-scripts` iframe can't be turned into an XSS vector.
///
/// CSS is intentionally preserved — both `<style>` blocks and inline `style=`
/// attributes — because the CSP already permits `style-src 'unsafe-inline'` and
/// email layout depends on it; CSS carries no script execution in WebKit. Inline
/// `data:image/...` sources are kept on `<img>` only; `data:` is stripped from
/// every other slot so a `data:text/html` href can't navigate the frame to an
/// un-CSP'd document. `cid:` refs are already rewritten to `/media/<key>` by the
/// time this runs, so relative URLs must pass through untouched.
pub(super) fn sanitize_email_html(source: &str) -> String {
    use std::borrow::Cow;

    let mut builder = ammonia::Builder::default();
    builder
        // Tags ammonia drops by default that email layout still leans on.
        .add_tags(["style", "font", "center"])
        // By default ammonia also empties `<style>` content; keep the CSS.
        .rm_clean_content_tags(["style"])
        // Ammonia sanitises a *fragment*: `<head>` and its children are dropped
        // as tags, but a dropped tag's text survives. `<title>` is the one head
        // element with text, so without this the subject line reappears as a
        // stray paragraph above the message body.
        .add_clean_content_tags(["title"])
        // Presentational attributes allowed on any element; styling must survive.
        .add_generic_attributes([
            "style",
            "class",
            "id",
            "align",
            "valign",
            "bgcolor",
            "background",
            "color",
            "face",
            "size",
            "width",
            "height",
            "border",
            "cellpadding",
            "cellspacing",
            "nowrap",
        ])
        // Apple Mail and Thunderbird mark a reply's quote as
        // `<blockquote type="cite">`; quote folding looks for it (see quote.rs).
        .add_tag_attributes("blockquote", ["type"])
        // `data:` is allowed through the scheme filter but then constrained to
        // image sources by the attribute filter below.
        .url_schemes(
            ["http", "https", "mailto", "tel", "data"]
                .into_iter()
                .collect(),
        )
        // `/media/<key>` inline-image refs are relative and must pass through.
        .url_relative(ammonia::UrlRelative::PassThrough)
        .attribute_filter(|element, attribute, value| {
            // `meron-*` ids and classes are the frames' own hooks: the injected
            // reader stylesheet, the code-block wrappers, the search marks. A
            // sender that claims one would be read back as ours.
            if matches!(attribute, "id" | "class") && value.to_ascii_lowercase().contains("meron-")
            {
                return None;
            }
            // A link without a scheme resolves against the app's own origin, so
            // following it would load an app page or a cached attachment (a
            // sender's `.html`, or a `cid:` link rewritten to `/media/<key>`)
            // inside the frame with the app's privileges. Only in-page
            // `#fragment` links stay.
            if attribute == "href" && !is_fragment_or_absolute_url(value) {
                return None;
            }
            if value.trim_start().to_ascii_lowercase().starts_with("data:") {
                // Keep `data:image/...` only on `<img src>`; drop it anywhere else
                // (e.g. an `href`) so it can't become a navigable script document.
                let ok = element == "img"
                    && attribute == "src"
                    && value
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("data:image/");
                return if ok { Some(Cow::Borrowed(value)) } else { None };
            }
            Some(Cow::Borrowed(value))
        });
    builder.clean(source).to_string()
}

/// True for an in-page `#fragment` or a URL that names its own scheme, read the
/// way a browser does: surrounding whitespace and controls trimmed, tabs and
/// newlines removed. Everything else (`/media/…`, `//host/…`, `x.html`) is
/// relative to the document and resolves against the app's origin.
pub(super) fn is_fragment_or_absolute_url(value: &str) -> bool {
    let value: String = value
        .trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    if value.starts_with('#') {
        return true;
    }
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// What a message declares on its own `<body>` for the whole page.
#[derive(Default)]
pub(super) struct BodyCanvas {
    pub(super) background: Option<String>,
    pub(super) text: Option<String>,
    /// The properties the declaration marked `!important`, named as the frames
    /// restore them: "background-color", "color".
    pub(super) important: Vec<&'static str>,
}

/// One canvas declaration read off a style attribute, with its importance.
pub(super) struct CanvasDeclaration {
    pub(super) value: String,
    pub(super) important: bool,
}

/// The canvas colors the email declares on its own `<body>`.
///
/// Ammonia sanitises a fragment, so the `<body>` tag (and with it the page colors
/// most HTML mail sets there) never survives into the rendered document. The
/// frames need them: a declared background marks the message as carrying its own
/// design, which is what keeps a dark theme from repainting it, and the text
/// color has to travel with it — restoring a black canvas without the white text
/// that went on it is worse than restoring neither (see `frameTheme.ts`). Both
/// are echoed into the head as `<meta>`s rather than re-applied here, so the
/// frontend stays the single place that decides.
pub(super) fn declared_body_colors(source: &str) -> BodyCanvas {
    let Some(tag) = body_start_tag(source) else {
        return BodyCanvas::default();
    };
    let style = attribute_value(tag, "style").unwrap_or_default();
    let (css_background, css_text) = css_canvas_colors(&style);
    // Importance travels with the color: the frames restore these as the inline
    // declarations they were, and an important one outranks a sender rule that a
    // normal one loses to.
    let mut important = Vec::new();
    if css_background.as_ref().is_some_and(|held| held.important) {
        important.push("background-color");
    }
    if css_text.as_ref().is_some_and(|held| held.important) {
        important.push("color");
    }

    // A style declaration outranks the presentational attribute, as it does in
    // the cascade — `bgcolor` is only the fallback for mail that predates CSS.
    // A declaration that isn't a color (an image layer, say) still wins, and
    // then drops out here rather than falling back to the attribute.
    let background = match css_background.as_ref().map(|held| held.value.as_str()) {
        Some(value) => safe_css_color(value),
        None => attribute_value(tag, "bgcolor")
            .as_deref()
            .and_then(safe_css_color),
    };
    let text = match css_text.as_ref().map(|held| held.value.as_str()) {
        Some(value) => safe_css_color(value),
        None => attribute_value(tag, "text")
            .as_deref()
            .and_then(safe_css_color),
    };

    BodyCanvas {
        background,
        text,
        important,
    }
}

/// The raw `<body ...>` start tag, skipping any that only appears inside a comment.
pub(super) fn body_start_tag(source: &str) -> Option<&str> {
    let lower = source.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("<body") {
        let start = from + rel;
        from = start + 5;
        // `<bodyfoo` is a different element; the tag name must end here.
        let after = lower.as_bytes().get(start + 5)?;
        if !matches!(after, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/') {
            continue;
        }
        // A `<body>` written inside a comment isn't the document's body. Every
        // comment that opens before it and hasn't closed yet swallows it.
        let commented = lower[..start]
            .rfind("<!--")
            .is_some_and(|open| !lower[open..start].contains("-->"));
        if commented {
            continue;
        }
        let end = start + lower[start..].find('>')?;
        return Some(&source[start..end]);
    }
    None
}

/// Accept a value only if it is an actual CSS color, and echo it back trimmed.
///
/// The value lands in a quoted attribute the frontend interpolates into CSS, so
/// the grammar is the boundary: a hex triplet, an `rgb()`/`hsl()` function, or a
/// bare keyword. Anything else — a `url()`, a gradient, a second declaration —
/// is dropped rather than escaped.
pub(super) fn safe_css_color(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 64 {
        return None;
    }

    let hex = value.strip_prefix('#');
    let is_hex = hex.is_some_and(|digits| {
        matches!(digits.len(), 3 | 4 | 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit())
    });
    // A keyword: `white`, `transparent`, `currentcolor`. No digits, so it can't
    // be confused with a length or a bare hex value.
    let is_keyword = value.chars().all(|c| c.is_ascii_alphabetic());
    // `rgb(0,0,0) url(probe.png)` must not slip through on a prefix/suffix check:
    // the arguments are matched whole, and may only be numbers and separators.
    let lower = value.to_ascii_lowercase();
    let is_function = ["rgb", "rgba", "hsl", "hsla"].iter().any(|name| {
        lower
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('('))
            .and_then(|rest| rest.strip_suffix(')'))
            .is_some_and(|args| {
                // Letters are the angle units a hue may carry (`180deg`,
                // `0.5turn`); parentheses stay out, which is what keeps a
                // trailing `url(...)` layer from passing as a color.
                !args.is_empty()
                    && args.chars().all(|c| {
                        c.is_ascii_alphanumeric()
                            || matches!(c, ',' | '.' | '%' | ' ' | '/' | '+' | '-')
                    })
            })
    });

    (is_hex || is_keyword || is_function).then(|| value.to_string())
}

/// Read `name="value"` (or `name=value`) out of a raw start tag, case-insensitively.
pub(super) fn attribute_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(name) {
        let at = from + rel;
        from = at + name.len();
        // Must be a whole attribute name: preceded by whitespace, followed by `=`.
        let before_ok = tag[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace);
        let rest = lower[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let value = tag[tag.len() - rest.len() + 1..].trim_start();
        let (quote, value) = match value.chars().next() {
            Some(q @ ('"' | '\'')) => (Some(q), &value[1..]),
            _ => (None, value),
        };
        let end = match quote {
            Some(q) => value.find(q)?,
            None => value.find(char::is_whitespace).unwrap_or(value.len()),
        };
        return Some(value[..end].to_string());
    }
    None
}

/// The canvas colors a style attribute ends up with: `(background, text)`.
///
/// Declarations are read in source order and the winner is chosen the way the
/// cascade chooses it: an `!important` declaration beats a normal one, and among
/// equals the last wins — including the `background` shorthand, which resets the
/// color even when it names an image, so a later `background: url(...)` really
/// does drop an earlier `background-color`.
pub(super) fn css_canvas_colors(
    style: &str,
) -> (Option<CanvasDeclaration>, Option<CanvasDeclaration>) {
    let mut background: Option<CanvasDeclaration> = None;
    let mut text: Option<CanvasDeclaration> = None;

    // Comments can sit anywhere, including between `!` and `important`, and can
    // hold whole declarations of their own.
    let style = strip_css_comments(style);
    for decl in style.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        // `!important` is part of the declaration, not of the color. CSS allows
        // whitespace after the `!`, which minifiers and hand-written mail both use.
        let (value, important) = split_important(value.trim());
        let slot = match name.trim().to_ascii_lowercase().as_str() {
            "background-color" | "background" => &mut background,
            "color" => &mut text,
            _ => continue,
        };
        // A normal declaration can't displace an important one; anything else
        // that comes later can.
        if slot
            .as_ref()
            .is_some_and(|held| held.important && !important)
        {
            continue;
        }
        *slot = Some(CanvasDeclaration {
            value: value.to_string(),
            important,
        });
    }

    (background, text)
}

/// Drop `/* ... */` comments, which are valid anywhere a space is.
pub(super) fn strip_css_comments(style: &str) -> String {
    let mut out = String::with_capacity(style.len());
    let mut rest = style;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        out.push(' ');
        match rest[open + 2..].find("*/") {
            Some(close) => rest = &rest[open + 2 + close + 2..],
            // An unterminated comment runs to the end of the declaration block.
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Split a declaration value from its `!important` flag, tolerating `! important`.
pub(super) fn split_important(value: &str) -> (&str, bool) {
    let Some(bang) = value.rfind('!') else {
        return (value, false);
    };
    let flag = value[bang + 1..].trim_start();
    // `get` rather than a slice: the suffix is untrusted UTF-8, and a fixed byte
    // offset can land inside a code point (`!ééééé`), which would panic.
    let important = flag
        .get(.."important".len())
        .is_some_and(|word| word.eq_ignore_ascii_case("important"))
        && flag["important".len()..].trim().is_empty();
    if important {
        (value[..bang].trim_end(), true)
    } else {
        (value, false)
    }
}

/// Wrap stored HTML for safe iframe rendering: inject a CSP `<meta>` that allows
/// same-origin (`/media`), data, and blob images/media always, and remote (`https:`)
/// images/media only when the account opts in. The reader renders this in a `sandbox`ed
/// iframe with `allow-scripts` (needed so the host's link-click listener fires under
/// WebKitGTK); the `default-src 'none'` CSP still blocks all email JS, so the
/// CSP also gates remote image/media loads.
pub fn prepare_html(source: &str, load_remote_images: bool) -> String {
    prepare_message_html(source, load_remote_images, "")
}

/// [`prepare_html`] for a mail with a subject, which is what tells an Outlook
/// forward from a reply when folding the quote (see quote.rs).
pub fn prepare_message_html(source: &str, load_remote_images: bool, subject: &str) -> String {
    // Defence in depth: strip script-bearing markup *before* the CSP `<meta>` is
    // injected, so a CSP bypass alone can't run the email's JS. CSS is kept (the
    // CSP allows `style-src 'unsafe-inline'`); only script vectors are removed.
    let raw_source = source;
    // Marked after sanitizing, which drops every sender `data-*` attribute, so
    // the quote marker the frames fold on is always ours. Bare URLs become
    // links on the sanitized markup, which the linkifier's scan relies on.
    let source = &crate::quote::mark_html_quote(
        &crate::linkify::linkify_html(&sanitize_email_html(source)),
        crate::quote::is_forward_subject(subject),
    );
    let (img, media) = if load_remote_images {
        (
            "'self' data: http: https:",
            "'self' data: blob: http: https:",
        )
    } else {
        ("'self' data:", "'self' data: blob:")
    };
    // `script-src`/`object-src`/`frame-src 'none'` are explicit for robustness
    // (they already inherit from `default-src`); `base-uri` and `form-action` do
    // NOT fall back to `default-src`, so they must be set to neutralise a `<base>`
    // hijack or a form posting out of the frame.
    let csp = format!(
        "default-src 'none'; script-src 'none'; object-src 'none'; frame-src 'none'; \
         base-uri 'none'; form-action 'none'; \
         img-src {img}; media-src {media}; style-src 'unsafe-inline'; font-src 'self' data:;"
    );
    // Keep oversized inline images within the reader width (preserving aspect), and
    // hint that they open the gallery. `style-src 'unsafe-inline'` covers this block.
    // The email's own canvas colors, hoisted off the `<body>` ammonia just
    // dropped. The frames read them back to tell a self-styled message (leave its
    // design alone) from one with no colors of its own (safe to render dark).
    let canvas = declared_body_colors(raw_source);
    let mut body_colors = [
        ("meron-body-bg", canvas.background),
        ("meron-body-fg", canvas.text),
    ]
    .into_iter()
    .filter_map(|(name, color)| {
        color.map(|color| format!("<meta name=\"{name}\" content=\"{color}\">"))
    })
    .collect::<String>();
    if !canvas.important.is_empty() {
        body_colors.push_str(&format!(
            "<meta name=\"meron-body-important\" content=\"{}\">",
            canvas.important.join(" ")
        ));
    }
    let head = format!(
        "<meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
         {body_colors}\
         <style>img,video{{max-width:100%;height:auto}}img{{cursor:zoom-in}}\
         a[{url_link}]{{word-break:break-all}}</style>",
        url_link = crate::linkify::URL_LINK_ATTR,
    );
    inject_head(source, &head)
}

/// Wrap a sanitized message in the document that carries `head_extra`.
///
/// Always a fresh shell, never a search for the message's own `<html>` or
/// `<head>`: ammonia returns a fragment, so those strings can only occur inside
/// its `<style>` text or an attribute value. Injecting there would park the CSP
/// `<meta>` where the browser does not read it. The desktop frame recognises
/// this exact shell (see `bubbleHtml.ts`), so keep the two in step.
pub(super) fn inject_head(html: &str, head_extra: &str) -> String {
    format!("<!doctype html><html><head>{head_extra}</head><body>{html}</body></html>")
}

/// Render stored HTML back into display markdown. Re-run by the store when a
/// render-version bump cleared `body` but the HTML source survives. (Plain-text
/// messages have no stored HTML — their `body` is never invalidated.)
pub fn render_body(html: &str) -> String {
    html_to_text(html)
}
