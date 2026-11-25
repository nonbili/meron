use super::*;

#[test]
fn message_id_reads_the_header_without_the_body() {
    // What waiting for a Sent copy matches on: the angle brackets come off,
    // a folded header still reads, and a message without the header (or one
    // that is not a message at all) yields nothing rather than a guess.
    let raw = b"From: a@example.com\r\nMessage-ID:\r\n <reply-1@meron>\r\nSubject: Re: Lunch\r\n\r\nbody\r\n";
    assert_eq!(message_id_of(raw), "reply-1@meron");
    assert_eq!(message_id_of(b"From: a@example.com\r\n\r\nbody\r\n"), "");
    assert_eq!(message_id_of(b""), "");
}

#[test]
fn preview_drops_markup_noise() {
    // Gmail's text/plain conventions, as sent by the Play Console.
    let body = "Your update is live\n\n[image: Google Play Console Logo] <https://play.google.com/console/>\n\nHello, Your update to Meron is live in the store.\n\n[image: Icon of your app Meron] *Meron*";
    assert_eq!(
        preview_of(body),
        "Your update is live Hello, Your update to Meron is live in the store. Meron"
    );
}

#[test]
fn preview_keeps_link_text_and_plain_punctuation() {
    let body = "# Heading\n\n---\n\n> quoted\n- See [the docs](https://example.com) and `code`.\nWrite to <bob@example.com>. 3 < 4 and [Attachment: a.pdf]";
    assert_eq!(
        preview_of(body),
        "Heading quoted See the docs and code. Write to bob@example.com. 3 < 4 and [Attachment: a.pdf]"
    );
}

#[test]
fn preview_keeps_unpaired_emphasis_and_nested_link_parens() {
    let body = "2 * 3 and *.rs stay; a lone ` too.\nSee [docs](https://example.com/Foo_(bar)) and *this* **too**.";
    assert_eq!(
        preview_of(body),
        "2 * 3 and *.rs stay; a lone ` too. See docs and this too."
    );
}

#[test]
fn preview_leaves_escaped_delimiters_alone() {
    let body = "Use \\*literal\\* and \\`ticks\\`, but \\\\*this* is markup.";
    assert_eq!(
        preview_of(body),
        "Use \\*literal\\* and \\`ticks\\`, but \\\\this is markup."
    );
}

#[test]
fn preview_truncates_long_bodies() {
    let body = "word ".repeat(100);
    let preview = preview_of(&body);
    assert_eq!(preview.chars().count(), 201);
    assert!(preview.ends_with('…'));
}

#[test]
fn converts_html_to_clean_text() {
    let html = "<html><head><style>body { color: red; }</style></head><body><h1>Hello World</h1><p>This is a <a href=\"https://example.com\">link</a>.<br>New line &amp; &lt;escaped&gt;.</p><div>Another block</div></body></html>";
    let text = html_to_text(html);
    assert_eq!(
        text,
        "Hello World\n\nThis is a [link](https://example.com).\nNew line & <escaped>.\n\nAnother block"
    );
}

#[test]
fn converts_html_images_to_image_markers() {
    let html = r#"<p><strong>Alice Example</strong> created an issue <a href="https://example.com/i">(example/project#266)</a></p><p><img alt="Screenshot.png" src="https://example.com/Screenshot.png"></p>"#;
    let text = html_to_text(html);
    assert_eq!(
        text,
        "**Alice Example** created an issue [(example/project#266)](https://example.com/i)\n\n[image: Screenshot.png]"
    );
}

#[test]
fn keeps_link_on_named_linked_image() {
    // A named image that is the sole content of an anchor (e.g. a Google logo
    // linking to a destination) keeps its link as `[image: alt](url)`.
    let html = r#"<p><a href="https://google.com"><img alt="Google" src="cid:logo"></a></p>"#;
    let text = html_to_text(html);
    assert_eq!(text, "[image: Google](https://google.com)");
}

#[test]
fn drops_decorative_icon_images() {
    // Empty-alt image (e.g. GitHub's issue-type icon) is dropped, leaving the
    // surrounding link intact rather than a stray "Image (view on web)".
    let html = r#"<p><a href="https://x/i"><img alt="" src="https://x/icon.png"> <strong>Alice Example</strong> created an issue (example/project#266)</a></p>"#;
    let text = html_to_text(html);
    assert_eq!(
        text,
        "[**Alice Example** created an issue (example/project#266)](https://x/i)"
    );
}

#[test]
fn captures_html_source_for_rerender() {
    let raw =
        b"From: a@b.com\r\nSubject: x\r\nContent-Type: text/html\r\n\r\n<p><strong>hi</strong></p>";
    let msg = parse_message(raw, None);
    let html = msg.body_html.as_deref().expect("html kept");
    assert!(html.contains("<strong>hi</strong>"));
    // Re-rendering the stored HTML reproduces the body.
    assert_eq!(render_body(html), msg.body);
    assert_eq!(msg.body, "**hi**");
}

#[test]
fn delivery_headers_mark_a_message_as_received() {
    // Written here (or filed to Sent by the server): no delivery headers.
    let sent = b"From: me@example.com\r\nSubject: x\r\n\r\nhi";
    assert!(!parse_message(sent, None).delivered);
    // The copy delivered to us carries the receiving MTA's headers, and
    // keeps them wherever it is filed afterwards.
    let received =
        b"Return-Path: <them@example.com>\r\nFrom: them@example.com\r\nSubject: x\r\n\r\nhi";
    assert!(parse_message(received, None).delivered);
    let routed = b"Delivered-To: me@example.com\r\nFrom: me@example.com\r\nSubject: x\r\n\r\nhi";
    assert!(parse_message(routed, None).delivered);
}

#[test]
fn truncated_base64_body_keeps_what_decodes() {
    // "hello world" is `aGVsbG8gd29ybGQ=`; cut mid-quantum it no longer
    // decodes as a whole.
    let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: multipart/alternative; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8gd29y\r\nbGQ\r\n--b--\r\n";
    let msg = parse_message(raw, None);
    assert_eq!(msg.body, "hello wor");
}

#[test]
fn undecodable_base64_plain_part_falls_back_to_html() {
    let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: multipart/alternative; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\naG\r\n--b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>hello world</p>\r\n--b--\r\n";
    let msg = parse_message(raw, None);
    assert!(msg.body_is_rendered);
    assert_eq!(msg.body, "hello world");
}

#[test]
fn plain_text_keeps_no_html_source() {
    let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: text/plain\r\n\r\nhello world";
    let msg = parse_message(raw, None);
    assert!(msg.body_html.is_none());
    assert_eq!(msg.body, "hello world");
}

#[test]
fn markup_in_the_plain_part_renders_as_html() {
    // A sender that stuffs its HTML into text/plain: the real HTML part
    // renders instead, so neither the reader nor the snippet shows tags.
    let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
<div style=\"font-family: Arial\"><p>plain markup</p></div>\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p><strong>html part</strong></p>\r\n\
--sep--\r\n";
    let msg = parse_message(raw, None);
    assert_eq!(msg.body, "**html part**");
    assert_eq!(msg.preview, "html part");

    // Prose that opens on a tag example keeps its plain part, tags and all.
    let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
<p>hello</p>\r\n\
\r\n\
Please replace <p> with <div> in this example.\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p>&lt;p&gt;hello&lt;/p&gt;</p>\r\n\
--sep--\r\n";
    let msg = parse_message(raw, None);
    assert!(msg.body.starts_with("<p>hello</p>"));
    assert!(msg.body.contains("replace <p> with <div>"));

    // A plain-only message is never reclassified, even when it is all tags.
    let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: text/plain\r\n\r\n\
<div style=\"font-family: Arial\"><p>hello</p></div>";
    let msg = parse_message(raw, None);
    assert!(msg.body_html.is_none());
    assert!(msg.body.starts_with("<div style="));
}

#[test]
fn plain_markup_needs_tags_to_dominate() {
    assert!(plain_is_markup(
        "<div style=\"font-family: Arial\">\n<p style=\"color: #1a2b3c\">Connect</p>\n</div>"
    ));
    // Tag examples in prose, even when the message ends on one.
    assert!(!plain_is_markup(
        "<p>hello</p>\n\nPlease replace <p> with <div> in this example."
    ));
    assert!(!plain_is_markup(
        "<p>hello</p>\n\nPlease replace <p> with <div>"
    ));
    assert!(!plain_is_markup("hello <b>world</b>"));
}

#[test]
fn multipart_alternative_prefers_plain_body_and_keeps_html() {
    let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
plain part\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p><strong>html part</strong></p>\r\n\
--sep--\r\n";
    let msg = parse_message(raw, None);
    assert_eq!(msg.body, "plain part");
    assert!(
        msg.body_html
            .as_deref()
            .unwrap_or_default()
            .contains("<strong>html part</strong>")
    );
}

#[test]
fn strips_style_blocks_with_attributes() {
    let html = "<html><head><style type=\"text/css\">body { color: red; }</style></head><body>Hello World</body></html>";
    let text = html_to_text(html);
    assert_eq!(text, "Hello World");
}

#[test]
fn decodes_rfc2047_subject() {
    // "=?UTF-8?B?SGVsbMO2?=" is base64 for "Hellö".
    assert_eq!(decode_words(" =?UTF-8?B?SGVsbMO2?="), "Hellö");
    assert_eq!(decode_words(" Plain subject"), "Plain subject");
}

fn lenient_from(raw: &str) -> (String, String) {
    let line = format!("From: {raw}\r\n\r\n");
    let (headers, _) = mailparse::parse_headers(line.as_bytes()).unwrap();
    from_of(&headers)
}

#[test]
fn decodes_encoded_words_glued_to_text() {
    // "GyRCJUYlOSVIGyhC" is ISO-2022-JP "テスト"; "Q2Fmw6k=" is UTF-8 "Café".
    assert_eq!(
        lenient_from("Acme=?iso-2022-jp?B?GyRCJUYlOSVIGyhC?= <news@example.com>"),
        ("Acmeテスト".to_string(), "news@example.com".to_string())
    );
    // Adjacent encoded-words join without the whitespace between them, even
    // when the first is glued to text.
    assert_eq!(
        lenient_from("Acme=?UTF-8?Q?Foo?= =?UTF-8?Q?Bar?= <a@example.com>").0,
        "AcmeFooBar"
    );
    assert_eq!(
        lenient_from("=?UTF-8?B?Q2Fmw6k=?= =?UTF-8?Q?_Bar?= Inc <a@example.com>").0,
        "Café Bar Inc"
    );
    // A name whose decoded text looks like an encoded-word is not decoded twice.
    // "PT9VVEYtOD9RP0NFTz89" is base64 for "=?UTF-8?Q?CEO?=".
    assert_eq!(
        lenient_from("=?UTF-8?B?PT9VVEYtOD9RP0NFTz89?= <a@example.com>").0,
        "=?UTF-8?Q?CEO?="
    );
    // Folded across lines.
    assert_eq!(
        lenient_from("Acme=?UTF-8?Q?Foo?=\r\n =?UTF-8?Q?Bar?= <a@example.com>").0,
        "AcmeFooBar"
    );
    // Glued on both sides, and two glued words back to back.
    assert_eq!(
        lenient_from("Acme=?UTF-8?Q?Foo?=Inc <a@example.com>").0,
        "AcmeFooInc"
    );
    assert_eq!(
        lenient_from("=?UTF-8?Q?Foo?==?UTF-8?Q?Bar?= <a@example.com>").0,
        "FooBar"
    );
    // Punctuation inside an encoded-word stays text, not address syntax.
    assert_eq!(
        lenient_from("=?UTF-8?Q?Doe,_Jane?= <jane@example.com>"),
        ("Doe, Jane".to_string(), "jane@example.com".to_string())
    );
    assert_eq!(
        lenient_from("Acme=?UTF-8?Q?_(News)?= <a@example.com>").0,
        "Acme (News)"
    );
    assert_eq!(
        lenient_from("=?UTF-8?Q?Acme_(News)?= <a@example.com>").0,
        "Acme (News)"
    );
    // Glued words inside a quoted name, or a name with a comment.
    assert_eq!(
        lenient_from("\"Acme=?UTF-8?Q?Foo?=\" <a@example.com>").0,
        "AcmeFoo"
    );
    assert_eq!(
        lenient_from("\"Acme \\\"Q\\\"=?UTF-8?Q?Foo?=\" <a@example.com>").0,
        "Acme \"Q\"Foo"
    );
    assert_eq!(
        lenient_from("Acme=?UTF-8?Q?Foo?= (News) <a@example.com>").0,
        "AcmeFoo"
    );
    assert_eq!(
        lenient_from("Acme=?UTF-8?Q?Foo?= (Ne\\)ws) Inc <a@example.com>").0,
        "AcmeFoo  Inc"
    );
    // Non-ASCII text before the encoded-word, glued or not.
    assert_eq!(
        lenient_from("Café=?UTF-8?Q?Foo?= <a@example.com>").0,
        "CaféFoo"
    );
    assert_eq!(
        lenient_from("Café =?UTF-8?Q?Foo?= <a@example.com>").0,
        "Café Foo"
    );
    // Encoded-word lookalikes inside an address are left alone.
    assert_eq!(
        lenient_from("User <user=?UTF-8?Q?foo?=@example.com>"),
        (
            "User".to_string(),
            "user=?UTF-8?Q?foo?=@example.com".to_string()
        )
    );
    assert_eq!(lenient_from("a =? b ?= c <a@example.com>").0, "a =? b ?= c");
    assert_eq!(lenient_from("Plain Name <a@example.com>").0, "Plain Name");
}

#[test]
fn splits_display_name_and_addr() {
    let (name, addr) = split_address("Maya Chen <maya@example.com>");
    assert_eq!(name, "Maya Chen");
    assert_eq!(addr, "maya@example.com");

    let (name, addr) = split_address("bare@example.com");
    assert_eq!(name, "");
    assert_eq!(addr, "bare@example.com");
}

#[test]
fn writes_inline_image_to_disk_and_keys_it() {
    // "AQID" is base64 for the bytes [1, 2, 3].
    let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/mixed; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p>hi</p>\r\n\
--sep\r\n\
Content-Type: image/png\r\n\
Content-Transfer-Encoding: base64\r\n\
Content-Disposition: inline; filename=\"a.png\"\r\n\
\r\n\
AQID\r\n\
--sep--\r\n";
    let root = std::env::temp_dir().join(format!("meron-media-test-{}", std::process::id()));
    let ctx = MediaCtx {
        root: root.clone(),
        account: "acct@host".to_string(),
        folder: "[Gmail]/All Mail".to_string(),
        uid: 42,
    };
    let msg = parse_message(raw, Some(&ctx));
    assert_eq!(msg.attachments.len(), 1);
    let key = msg.attachments[0].key.as_deref().expect("image keyed");
    // Unsafe path chars in account/folder are flattened to `_`.
    assert_eq!(key, "acct_host/_Gmail__All_Mail/42/0.png");
    let bytes = std::fs::read(root.join(key)).expect("file written");
    assert_eq!(bytes, vec![1, 2, 3]);
    let _ = std::fs::remove_dir_all(&root);
}

// An inline image without a filename (Meron's own composer sends them so)
// is keyed by its MIME type. `image/svg+xml` must give `.svg`, the type the
// desktop's file server knows: `.svg_xml` was served as text/plain, and a
// browser doesn't draw an SVG of that type.
#[test]
fn an_inline_svg_without_filename_is_keyed_svg() {
    let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/related; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p><img src=\"cid:logo@x\"></p>\r\n\
--sep\r\n\
Content-Type: image/svg+xml\r\n\
Content-Disposition: inline\r\n\
Content-ID: <logo@x>\r\n\
\r\n\
<svg xmlns=\"http://www.w3.org/2000/svg\"/>\r\n\
--sep--\r\n";
    let root = std::env::temp_dir().join(format!("meron-svg-key-test-{}", std::process::id()));
    let ctx = MediaCtx {
        root: root.clone(),
        account: "acct".to_string(),
        folder: "INBOX".to_string(),
        uid: 7,
    };
    let msg = parse_message(raw, Some(&ctx));
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(msg.attachments.len(), 1);
    assert_eq!(
        msg.attachments[0].key.as_deref(),
        Some("acct/INBOX/7/0.svg")
    );
}

#[test]
fn rewrites_cid_refs_in_html_source_to_media_keys() {
    let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/related; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p><img src=\"cid:logo123\"></p>\r\n\
--sep\r\n\
Content-Type: image/png\r\n\
Content-Transfer-Encoding: base64\r\n\
Content-ID: <logo123>\r\n\
Content-Disposition: inline; filename=\"a.png\"\r\n\
\r\n\
AQID\r\n\
--sep--\r\n";
    let root = std::env::temp_dir().join(format!("meron-cid-test-{}", std::process::id()));
    let ctx = MediaCtx {
        root: root.clone(),
        account: "acct".to_string(),
        folder: "inbox".to_string(),
        uid: 7,
    };
    let msg = parse_message(raw, Some(&ctx));
    let html = msg.body_html.as_deref().expect("html kept");
    assert!(html.contains("/media/acct/inbox/7/0.png"));
    assert!(!html.contains("cid:logo123"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn prepare_html_keeps_the_csp_ahead_of_markup_posing_as_a_head() {
    // `<style>` text and attribute values survive sanitizing verbatim.
    for source in [
        "<style>/*<html><head>*/</style><img src=\"https://t.example/p.png\">",
        "<img alt=\"<html><head>\" src=\"https://t.example/p.png\">",
    ] {
        let out = prepare_html(source, false);
        let head = "<!doctype html><html><head><meta charset=\"utf-8\">\
                    <meta http-equiv=\"Content-Security-Policy\"";
        assert!(out.starts_with(head), "{out}");
        assert_eq!(out.matches("Content-Security-Policy").count(), 1, "{out}");
    }
}

#[test]
fn prepare_html_gates_remote_images_via_csp() {
    let off = prepare_html("<p>hi</p>", false);
    assert!(off.contains("Content-Security-Policy"));
    assert!(off.contains("img-src 'self' data:;"));
    assert!(off.contains("media-src 'self' data: blob:;"));
    assert!(!off.contains("https:"));

    let on = prepare_html("<html><head></head><body>hi</body></html>", true);
    assert!(on.contains("img-src 'self' data: http: https:;"));
    assert!(on.contains("media-src 'self' data: blob: http: https:;"));
}

#[test]
fn prepare_html_strips_script_vectors_keeps_styling() {
    let out = prepare_html(
        "<style>p{color:red}</style>\
         <p style=\"font-weight:bold\" onclick=\"steal()\">hi</p>\
         <script>alert(1)</script>\
         <a href=\"javascript:alert(2)\">x</a>\
         <a href=\"https://example.com\">ok</a>\
         <iframe src=\"https://evil\"></iframe>\
         <form action=\"https://evil\"></form>",
        false,
    );
    // Script-bearing markup is gone.
    assert!(!out.contains("<script"));
    assert!(!out.contains("onclick"));
    assert!(!out.contains("javascript:"));
    assert!(!out.contains("<iframe"));
    assert!(!out.contains("<form"));
    // Styling and safe links survive.
    assert!(out.contains("<style>"));
    assert!(out.contains("color:red"));
    assert!(out.contains("font-weight:bold"));
    assert!(out.contains("https://example.com"));
}

#[test]
fn prepare_html_marks_only_its_own_quotes() {
    // A sender can't fold part of a message by writing the marker itself.
    let forged = prepare_html(r#"<p data-meron-quote="">hidden?</p><p>tail</p>"#, false);
    assert!(!forged.contains("data-meron-quote"), "{forged}");
    let reply = prepare_html(
        r#"<p>Yes</p><div class="gmail_quote">On Mon, Jane wrote:<blockquote>Lunch?</blockquote></div>"#,
        false,
    );
    assert!(
        reply.contains(r#"class="gmail_quote" data-meron-quote="""#),
        "{reply}"
    );
    // Apple Mail / Thunderbird: the `type="cite"` the detector keys on has
    // to survive sanitizing.
    let cite = prepare_html(
        r#"<p>Yes</p><div class="moz-cite-prefix">On 9/1/25 Jane wrote:</div><blockquote type="cite"><p>Lunch?</p></blockquote>"#,
        false,
    );
    assert!(
        cite.contains(r#"<blockquote type="cite" data-meron-quote="">"#),
        "{cite}"
    );
    assert!(
        cite.contains(r#"class="moz-cite-prefix" data-meron-quote="""#),
        "{cite}"
    );
}

#[test]
fn prepare_html_drops_head_title_text() {
    let out = prepare_html(
        "<html><head><title>Secure your schedule</title></head><body><p>hi</p></body></html>",
        false,
    );
    // The subject must not leak into the body as a stray line of text.
    assert!(!out.contains("Secure your schedule"));
    assert!(out.contains("hi"));
}

#[test]
fn prepare_html_hoists_body_colors() {
    let style = prepare_html(
        "<html><body style=\"margin: 0; background-color: #f5f4f2;\"><p>hi</p></body></html>",
        false,
    );
    assert!(style.contains("<meta name=\"meron-body-bg\" content=\"#f5f4f2\">"));

    // A dark canvas travels with the text color that makes it readable.
    let dark = prepare_html(
        "<html><body bgcolor=\"#000000\" text=\"white\"><p>hi</p></body></html>",
        false,
    );
    assert!(dark.contains("<meta name=\"meron-body-bg\" content=\"#000000\">"));
    assert!(dark.contains("<meta name=\"meron-body-fg\" content=\"white\">"));

    // A style declaration outranks the presentational attribute.
    let both = prepare_html(
        "<body bgcolor=\"#ffffff\" style=\"background: #101010\"><p>hi</p></body>",
        false,
    );
    assert!(both.contains("content=\"#101010\""));

    // A `<body>` that only appears inside a comment isn't the document's.
    let commented = prepare_html(
        "<!-- <body bgcolor=\"#123456\"> --><body bgcolor=\"#abcdef\"><p>hi</p></body>",
        false,
    );
    assert!(commented.contains("content=\"#abcdef\""));
    assert!(!commented.contains("#123456"));

    assert!(!prepare_html("<body><p>hi</p></body>", false).contains("meron-body-"));
}

#[test]
fn prepare_html_hoists_colors_not_arbitrary_css() {
    // Only real colors travel: an image layer would become a background-image
    // (and a network fetch) once the frontend interpolates it into CSS.
    for value in [
        "background: url(probe.png)",
        "background: linear-gradient(#fff, #000)",
        "background-color: a&quot;><script>",
        "background-color: expression(alert(1))",
    ] {
        let out = prepare_html(&format!("<body style=\"{value}\"><p>hi</p></body>"), false);
        assert!(!out.contains("meron-body-bg"), "accepted {value}");
    }

    // A second declaration is just another declaration, not an injection.
    let pair = prepare_html(
        "<body style=\"background-color: #fff; color: red\"><p>hi</p></body>",
        false,
    );
    assert!(pair.contains("<meta name=\"meron-body-bg\" content=\"#fff\">"));
    assert!(pair.contains("<meta name=\"meron-body-fg\" content=\"red\">"));

    // `!important` belongs to the declaration, and decides which one wins.
    let important = prepare_html(
        "<body style=\"background: #000000 !important; background: #ffffff; color: white\"><p>hi</p></body>",
        false,
    );
    assert!(important.contains("<meta name=\"meron-body-bg\" content=\"#000000\">"));
    assert!(important.contains("<meta name=\"meron-body-fg\" content=\"white\">"));

    // CSS allows whitespace after the `!`, and a comment anywhere a space is.
    for style in [
        "background: #000000 ! important; background: #ffffff",
        "background: #000000 !IMPORTANT; background: #ffffff",
        "background: #000000 !/* whoops */important; background: #ffffff",
        // Non-ASCII after the `!` must not land mid-code-point.
        "background: #000000; background: #ffffff !ééééé; background: #000000",
        "background: #ffffff; /* background: #ffffff */ background: #000000",
    ] {
        let out = prepare_html(&format!("<body style=\"{style}\"><p>hi</p></body>"), false);
        assert!(
            out.contains("<meta name=\"meron-body-bg\" content=\"#000000\">"),
            "lost the winning declaration in {style}"
        );
    }

    // A trailing layer is not a color, however the value starts.
    for value in [
        "background: rgb(0,0,0) url(probe.png)",
        "background-color: rgb(0 0 0 / url(x))",
        "background: hsl(0, 0%, 0%) no-repeat",
    ] {
        let out = prepare_html(&format!("<body style=\"{value}\"><p>hi</p></body>"), false);
        assert!(!out.contains("meron-body-bg"), "accepted {value}");
    }

    // The last declaration wins, as it does in the cascade: a shorthand that
    // names an image resets an earlier color rather than losing to it.
    let reset = prepare_html(
        "<body style=\"background-color: #fff; background: url(x.png); color: white\"><p>hi</p></body>",
        false,
    );
    assert!(!reset.contains("meron-body-bg"));
    assert!(reset.contains("<meta name=\"meron-body-fg\" content=\"white\">"));

    // Importance travels with the declaration it was written on.
    let important = prepare_html(
        "<body style=\"background: #000 !important; color: white\"><p>hi</p></body>",
        false,
    );
    assert!(
        important.contains("<meta name=\"meron-body-important\" content=\"background-color\">")
    );
    assert!(
        !prepare_html("<body style=\"color: white\"><p>hi</p></body>", false)
            .contains("meron-body-important")
    );

    // `#fff`, `rgb(...)` and a bare keyword are the accepted forms — including
    // the angle units a hue may carry.
    for value in [
        "#fff",
        "#ffffffaa",
        "rgb(255, 0, 0)",
        "white",
        "hsl(180deg 50% 50%)",
        "hsl(0.5turn 50% 50%)",
        "hsl(-90 50% 50%)",
    ] {
        let out = prepare_html(
            &format!("<body style=\"background-color: {value}\"><p>hi</p></body>"),
            false,
        );
        assert!(
            out.contains(&format!("content=\"{value}\"")),
            "dropped {value}"
        );
    }
}

#[test]
fn prepare_html_marks_a_gmail_reply_with_signatures_above_and_below_the_quote() {
    // Gmail's reply with the signature under the quote, which itself holds
    // the quoted message's own signature.
    let out = prepare_html(
        r#"<div dir="ltr">x1</div><br><div class="gmail_quote gmail_quote_container"><div dir="ltr" class="gmail_attr">On Thu, Sep 10, 2026 at 5:11 PM Alex &lt;<a href="mailto:a@b.c">a@b.c</a>&gt; wrote:<br></div><blockquote class="gmail_quote"><div dir="ltr"><div>t2</div><span class="gmail_signature_prefix">-- </span><br><div dir="ltr" class="gmail_signature">2t</div></div></blockquote></div><div><br clear="all"></div><span class="gmail_signature_prefix">-- </span><br><div dir="ltr" class="gmail_signature"><div dir="ltr">s2</div></div>"#,
        false,
    );
    assert!(
        out.contains(r#"<div class="gmail_quote gmail_quote_container" data-meron-quote="">"#),
        "{out}"
    );
    assert!(
        out.contains(r#"<div class="gmail_signature" data-meron-quote="">"#),
        "{out}"
    );
}

#[test]
fn prepare_html_lets_url_links_break_anywhere() {
    let out = prepare_html(
        r#"<p>&lt;https://x.example/a&gt; and <a href="https://y.example/">https://y.example/</a> or <a href="https://z.example/">docs</a></p>"#,
        false,
    );
    assert!(
        out.contains("a[data-meron-url]{word-break:break-all}"),
        "{out}"
    );
    assert_eq!(
        out.matches(r#" data-meron-url="">https://"#).count(),
        2,
        "{out}"
    );
    assert!(
        out.contains(r#"<a href="https://z.example/" rel="noopener noreferrer">docs</a>"#),
        "{out}"
    );
    // A sender's own attribute is dropped, so a prose link cannot claim it.
    let planted = prepare_html(
        r#"<a href="https://z.example/" data-meron-url="">docs</a>"#,
        false,
    );
    assert!(
        !planted.split_once("<body>").unwrap().1.contains("meron-"),
        "{planted}"
    );
}

#[test]
fn prepare_html_strips_sender_claimed_meron_hooks() {
    // The frames read their own `meron-*` ids and classes back out of the
    // document; a sender must not be able to plant one.
    let out = prepare_html(
        "<style id=\"meron-reader-style\">body{color:#333}</style>\
         <mark class=\"meron-search-hit\">x</mark>\
         <div class=\"card meron-copy-code\">y</div>",
        false,
    );
    let body = out.split_once("<body>").expect("shell").1;
    assert!(!body.contains("meron-"), "{body}");
    // Only the hooks are dropped, not the elements or their other markup.
    assert!(out.contains("<style>"));
    assert!(out.contains("<mark>"));
}

#[test]
fn prepare_html_keeps_data_image_but_strips_data_href() {
    let out = prepare_html(
        "<img src=\"data:image/png;base64,AAAA\">\
         <a href=\"data:text/html,<script>alert(1)</script>\">x</a>\
         <img src=\"/media/acct/inbox/1/0.png\">",
        true,
    );
    assert!(out.contains("data:image/png;base64,AAAA"));
    assert!(out.contains("/media/acct/inbox/1/0.png"));
    // The data:text/html navigation vector is dropped, href and all.
    assert!(!out.contains("data:text/html"));
}

#[test]
fn prepare_html_strips_links_that_resolve_against_the_app() {
    let out = prepare_html(
        "<a href=\"/media/acct/inbox/1/0.html\">a</a>\
         <a href=\"//wails/media/x.html\">b</a>\
         <a href=\"0.html\">c</a>\
         <a href=\"\t/wails/runtime\">d</a>\
         <map><area href=\"/media/acct/inbox/1/0.html\"></map>\
         <a href=\"#top\">e</a>\
         <a href=\"https://example.com/x\">f</a>\
         <a href=\"mailto:a@example.com\">g</a>\
         <img src=\"/media/acct/inbox/1/1.png\">",
        false,
    );
    assert!(!out.contains(".html"), "{out}");
    assert!(!out.contains("/wails"), "{out}");
    assert!(out.contains("href=\"#top\""), "{out}");
    assert!(out.contains("href=\"https://example.com/x\""), "{out}");
    assert!(out.contains("href=\"mailto:a@example.com\""), "{out}");
    // Inline images still load from the media cache.
    assert!(out.contains("src=\"/media/acct/inbox/1/1.png\""), "{out}");
}

#[test]
fn a_cid_link_to_an_attachment_does_not_survive_rendering() {
    let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/related; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p><a href=\"cid:page\">open</a></p>\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
Content-ID: <page>\r\n\
Content-Disposition: attachment; filename=\"page.html\"\r\n\
\r\n\
<script src=\"1.js\"></script>\r\n\
--sep--\r\n";
    let root = std::env::temp_dir().join(format!("meron-cid-link-test-{}", std::process::id()));
    let ctx = MediaCtx {
        root: root.clone(),
        account: "acct".to_string(),
        folder: "inbox".to_string(),
        uid: 7,
    };
    let msg = parse_message(raw, Some(&ctx));
    let _ = std::fs::remove_dir_all(&root);
    let html = msg.body_html.as_deref().expect("html kept");
    // The rewrite still points the link at the cached page...
    assert!(html.contains("href=\"/media/acct/inbox/7/"), "{html}");
    // ...and rendering drops it.
    let out = prepare_html(html, false);
    assert!(out.contains(">open</a>"), "{out}");
    assert!(!out.contains("/media/"), "{out}");
}

#[test]
fn cached_media_available_requires_keyed_files() {
    let root = std::env::temp_dir().join(format!(
        "meron-media-availability-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("acct/inbox/1")).unwrap();
    std::fs::write(root.join("acct/inbox/1/0.png"), [1, 2, 3]).unwrap();

    let mut msg = Message::default();
    msg.attachments.push(Attachment {
        filename: "a.png".to_string(),
        mime: "image/png".to_string(),
        size: 3,
        key: Some("acct/inbox/1/0.png".to_string()),
        oversized: false,
    });
    assert!(cached_media_available(&root, &msg));

    std::fs::remove_file(root.join("acct/inbox/1/0.png")).unwrap();
    assert!(!cached_media_available(&root, &msg));
    assert_eq!(missing_media_count(&root, &msg), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn has_attachments_ignores_images_the_html_shows_inline() {
    let attachment = |key: &str| Attachment {
        filename: "x".to_string(),
        mime: "image/png".to_string(),
        size: 3,
        key: Some(key.to_string()),
        oversized: false,
    };
    let mut msg = Message {
        body_html: Some("<img src=\"/media/acct/inbox/1/0.png\">".to_string()),
        ..Default::default()
    };
    assert!(!msg.has_attachments());

    msg.attachments.push(attachment("acct/inbox/1/0.png"));
    assert!(!msg.has_attachments(), "a cid: logo is not an attachment");

    msg.attachments.push(attachment("acct/inbox/1/1.pdf"));
    assert!(msg.has_attachments());

    // Plain-text mail has no HTML to show anything inline.
    let mut plain = Message::default();
    plain.attachments.push(attachment("acct/inbox/2/0.png"));
    assert!(plain.has_attachments());
}

#[test]
fn named_calendar_alternative_is_not_a_duplicate_attachment() {
    let raw = "Content-Type: multipart/mixed; boundary=outer

--outer
Content-Type: multipart/alternative; boundary=inner

--inner
Content-Type: text/plain; name=body.txt

Invitation
--inner
Content-Type: text/calendar; name=invite.ics
Content-Disposition: inline; filename=invite.ics

BEGIN:VCALENDAR
END:VCALENDAR
--inner--
--outer
Content-Type: text/calendar; name=invite.ics
Content-Disposition: attachment; filename=invite.ics

BEGIN:VCALENDAR
END:VCALENDAR
--outer--
"
    .replace('\n', "\r\n");
    let message = parse_message(raw.as_bytes(), None);
    let files: Vec<_> = message.file_attachments().collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename, "invite.ics");
    assert_eq!(files[0].mime, "text/calendar");
}

#[test]
fn a_failed_write_is_recovered_and_an_oversized_file_is_not_written() {
    let root = std::env::temp_dir().join(format!("meron-unwritable-{}", std::process::id()));
    let _ = std::fs::remove_file(&root);
    std::fs::write(&root, b"not a directory").unwrap();
    let ctx = MediaCtx {
        root: root.clone(),
        account: "acc".into(),
        folder: "INBOX".into(),
        uid: 1,
    };
    let raw = b"Message-ID: <one@test>\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=file.bin\r\n\r\ndata";
    let message = parse_message(raw, Some(&ctx));
    assert_eq!(message.attachments.len(), 1);
    assert_eq!(message.attachments[0].key, None);
    assert!(!message.attachments[0].oversized);
    assert_eq!(missing_media_count(&root, &message), 1);
    std::fs::remove_file(&root).unwrap();
    assert_eq!(
        write_media_with_cap(&ctx, 0, "file.bin", "application/octet-stream", b"data", 3),
        None
    );
    assert!(!root.exists());
}

#[test]
fn keyless_attachments_trigger_media_recovery_unless_oversized() {
    // A row cached before attachments were persisted has a key-less file;
    // it must report unavailable so the caller refetches and writes it.
    // One left off disk for its size would only be left off again.
    let root = std::env::temp_dir().join(format!("meron-keyless-{}", std::process::id()));
    let mut msg = Message::default();
    msg.attachments.push(Attachment {
        filename: "invite.ics".to_string(),
        mime: "text/calendar".to_string(),
        size: 3072,
        key: None,
        oversized: false,
    });
    assert!(!cached_media_available(&root, &msg));
    assert_eq!(missing_media_count(&root, &msg), 1);
    msg.attachments[0].oversized = true;
    assert!(cached_media_available(&root, &msg));
    assert_eq!(missing_media_count(&root, &msg), 0);
}

#[test]
fn parses_multipart_message_summary() {
    let raw = b"From: Maya Chen <maya@example.com>\r\n\
Subject: =?UTF-8?B?TGF1bmNoIG5vdGVz?=\r\n\
Date: Mon, 18 May 2026 09:42:00 +0000\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
The launch notes are ready for review.\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p><strong>The launch notes</strong> are ready for review.</p>\r\n\
--sep--\r\n";
    let msg = parse_message(raw, None);
    assert_eq!(msg.subject, "Launch notes");
    assert_eq!(msg.from_name, "Maya Chen");
    assert_eq!(msg.from_addr, "maya@example.com");
    assert_eq!(msg.body, "The launch notes are ready for review.");
    assert!(
        msg.body_html
            .as_deref()
            .unwrap_or_default()
            .contains("<strong>The launch notes</strong>")
    );
    assert!(msg.preview.starts_with("The launch notes are ready"));
}

#[test]
fn flattens_layout_tables_into_blocks() {
    // Email layout tables must not become markdown tables: paragraphs in a
    // cell stay separate blocks, and there are no `|`/`---` table artifacts.
    let html = "<table><tr><td><p>First paragraph.</p><p>Second paragraph.</p></td></tr></table>";
    let text = html_to_text(html);
    assert_eq!(text, "First paragraph.\n\nSecond paragraph.");
    assert!(!text.contains('|'));
}

#[test]
fn normalizes_whitespace_and_thin_spaces() {
    let input = "ZHANG XINGYAO\n\n\u{200a}\n\nAmount\n\n6,383.25 USD\n\n\u{200b}\n\nDate";
    let normalized = normalize_text(input);
    assert_eq!(
        normalized,
        "ZHANG XINGYAO\n\nAmount\n\n6,383.25 USD\n\nDate"
    );
}

#[test]
fn drops_empty_linked_images() {
    // A linked image with no alt text (e.g. social media icon links in Airwallex emails)
    // should have the entire empty link dropped rather than leaving a stray `[](url)`.
    let html = r#"<p><a href="https://facebook.com/airwallex"><img alt="" src="https://example.com/Facebook.png"></a></p>"#;
    let text = html_to_text(html);
    assert_eq!(text, "");
}

#[test]
fn reads_the_envelope_of_an_outgoing_message() {
    let raw = concat!(
        "From: Me <Me@Example.com>\r\n",
        "To: \"B\" <b@example.com>, c@example.com\r\n",
        "Cc: A@Example.com\r\n",
        "Bcc: hidden@example.com\r\n",
        "Subject: =?utf-8?q?Re=3A_lunch?=\r\n",
        "Message-ID: <reply-1@meron>\r\n",
        "Date: Tue, 14 Nov 2023 22:13:20 +0000\r\n",
        "\r\n",
        "body\r\n",
    );
    let envelope = sent_envelope_of(raw.as_bytes());
    assert_eq!(envelope.message_id, "reply-1@meron");
    assert_eq!(envelope.subject, "Re: lunch");
    assert_eq!(envelope.from_addr, "Me@Example.com");
    // To + Cc, lowercased and sorted; Bcc stays out — the sending server
    // strips it, so the copy that comes back never carries it.
    assert_eq!(
        envelope.recipients,
        vec![
            "a@example.com".to_string(),
            "b@example.com".to_string(),
            "c@example.com".to_string()
        ]
    );
    assert_eq!(envelope.date, 1_700_000_000);
}

#[test]
fn an_unparseable_message_has_a_blank_envelope() {
    assert_eq!(
        sent_envelope_of(b"\x00not a message"),
        SentEnvelope::default()
    );
}
