//! Finding the quoted tail of a reply — the earlier conversation a mail client
//! pastes under the new text — so the conversation view can fold it away the
//! way Gmail does. Every bubble sits right under the message it answers, so an
//! unfolded quote repeats the thread above it.
//!
//! Only a quote that ends the message is folded: one the sender answered inline
//! is part of the reply. A message that is nothing but a quote, and a forward
//! (whose quoted part *is* the content), are left alone.

use ego_tree::iter::Edge;
use ego_tree::{NodeId, NodeRef};
use scraper::{ElementRef, Html, Node, Selector};

/// The attribute [`mark_html_quote`] puts on the quoted tail's elements. The
/// sanitizer drops every `data-*` attribute a sender writes, so an element
/// carrying it was marked here.
pub const HTML_QUOTE_ATTR: &str = "data-meron-quote";

/// Quote containers the common clients write: Gmail's `gmail_quote`, Apple
/// Mail's and Thunderbird's `<blockquote type="cite">`, Yahoo's `yahoo_quoted`.
const HTML_QUOTE_SELECTOR: &str = "div.gmail_quote, blockquote[type=cite i], div.yahoo_quoted";

/// Where Outlook's quote starts. It writes no container: the earlier mail's
/// headers ("From: … Sent: … Subject: …") go in a block of their own and the
/// mail itself follows as that block's siblings, to the end of the message.
/// Outlook on the web, the new Outlook and Outlook.com give the block an id --
/// prefixed (`x_divRplyFwdMsg`) once it is quoted again; the Word-based desktop
/// Outlook only rules it off with a top border (see [`is_outlook_header`]).
const OUTLOOK_HEADER_SELECTOR: &str = r#"div[id$="divRplyFwdMsg"], div[style*="border-top"]"#;

/// What Outlook on the web leaves on the elements the sender typed into. In a
/// quoted mail it is prefixed like the ids (`x_elementToProof`), so the bare
/// class under the header block is an answer written into the quote.
const OUTLOOK_TYPED_CLASS: &str = "elementToProof";

/// Longest attribution line ("On Mon, … Jane <jane@example.com> wrote:") taken
/// as one; anything longer is a paragraph that happens to end in a colon.
const MAX_ATTRIBUTION_CHARS: usize = 300;

/// Where the quoted tail of a plain body starts — its attribution line when it
/// has one — as a UTF-16 offset into `body`, the unit JavaScript and Kotlin
/// index strings by. `None` when there is nothing to fold.
pub fn plain_quote_start(body: &str) -> Option<usize> {
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        lines.push((offset, line.trim_end_matches(['\n', '\r'])));
        offset += line.len();
    }
    let blank = |index: usize| lines[index].1.trim().is_empty();
    let quoted = |index: usize| lines[index].1.trim_start().starts_with('>');

    let mut end = lines.len();
    while end > 0 && blank(end - 1) {
        end -= 1;
    }
    // Gmail puts the sender's signature ("-- " and what follows) under the
    // quote. It folds away with the quote rather than hiding the quote from us.
    if let Some(delimiter) = (0..end)
        .rev()
        .find(|&index| lines[index].1.trim_end() == "--")
        && (delimiter..end).all(|index| !quoted(index))
    {
        end = delimiter;
        while end > 0 && blank(end - 1) {
            end -= 1;
        }
    }
    // Walk up through the quoted run. Blank lines inside it belong to it only
    // when more quoted lines sit above them.
    let mut start = end;
    let mut cursor = end;
    while cursor > 0 {
        if quoted(cursor - 1) {
            cursor -= 1;
            start = cursor;
        } else if blank(cursor - 1) {
            cursor -= 1;
        } else {
            break;
        }
    }
    if start == end {
        return None;
    }

    // The attribution sits just above the quote, at most one blank line away.
    let mut above = start;
    if above > 0 && blank(above - 1) {
        above -= 1;
    }
    if above > 0 && is_attribution(lines[above - 1].1) {
        let line = above - 1;
        start = line;
        // Gmail wraps a long attribution at 78 columns, often inside the
        // address: "… Jane Doe <" / "jane@example.com> wrote:". Take the first
        // half too when the second is plainly a continuation of it.
        if line > 0
            && !blank(line - 1)
            && is_attribution_continuation(lines[line - 1].1, lines[line].1)
        {
            start = line - 1;
        }
        let attribution: String = lines[start..=line].iter().map(|(_, text)| *text).collect();
        if is_forward_header(&attribution) {
            return None;
        }
    }

    let byte = lines[start].0;
    if body[..byte].trim().is_empty() {
        return None;
    }
    Some(body[..byte].encode_utf16().count())
}

fn is_attribution(line: &str) -> bool {
    let line = line.trim();
    (line.ends_with(':') || line.ends_with('：'))
        && !line.starts_with('>')
        && line.chars().count() <= MAX_ATTRIBUTION_CHARS
}

/// Whether `second` is the wrapped end of an attribution that starts on `first`.
/// Only a break inside or right after the sender's address counts: a short
/// attribution ("Jane wrote:") on its own says nothing about the line above it,
/// which is usually the end of the reply.
fn is_attribution_continuation(first: &str, second: &str) -> bool {
    let (first, second) = (first.trim(), second.trim());
    if first.starts_with('>') {
        return false;
    }
    // "… Jane Doe <" / "jane@example.com> wrote:" — the address opens on the
    // first line and closes on the second. A complete `<address>` on the second
    // line is an attribution of its own, not the end of the line above.
    let opens_address = match (first.rfind('<'), first.rfind('>')) {
        (Some(open), Some(close)) => open > close,
        (Some(_), None) => true,
        _ => false,
    };
    let split_inside_address = opens_address
        && second
            .split_whitespace()
            .next()
            .is_some_and(|word| !word.starts_with('<') && word.ends_with('>'));
    // "… Jane Doe <jane@example.com>" / "wrote:", or with the date after the
    // address, as localized Gmail writes it: "Jane <jane@example.com>, 29 Eyl
    // 2026 Sal, 14:46 tarihinde şunu" / "yazdı:". The comma right after the
    // address and the time in the date tell it from a sentence that happens
    // to hold an address.
    let after_address = first
        .find('@')
        .and_then(|at| first[at..].rfind('>').map(|close| &first[at + close + 1..]));
    let has_time = |text: &str| {
        let chars: Vec<char> = text.chars().collect();
        chars.windows(4).any(|w| {
            w[0].is_ascii_digit() && w[1] == ':' && w[2].is_ascii_digit() && w[3].is_ascii_digit()
        })
    };
    let split_after_address = after_address.is_some_and(|rest| {
        rest.is_empty()
            || (rest.starts_with(',') && has_time(rest) && !rest.ends_with(['.', '!', '?']))
    }) && second.split_whitespace().count() <= 2;
    split_inside_address || split_after_address
}

/// A forward's header ("---------- Forwarded message ---------", Apple's "Begin
/// forwarded message:", Outlook's "-----Original Message-----"): what follows
/// is the content being passed on, not a reply's history.
fn is_forward_header(text: &str) -> bool {
    let text = text.trim_start().to_lowercase();
    text.starts_with("-----") || text.contains("forwarded message")
}

/// Whether a subject says the mail is a forward ("Fw:", "Fwd:", and what the
/// localized Outlooks write for it), going by its first prefix: the last thing
/// done to the mail.
pub fn is_forward_subject(subject: &str) -> bool {
    let Some((prefix, _)) = subject.split_once([':', '：']) else {
        return false;
    };
    matches!(
        prefix.trim().to_lowercase().as_str(),
        "fw" | "fwd" | "tr" | "wg" | "rv" | "enc" | "i" | "doorst" | "vl" | "pd" | "转发" | "轉寄"
    )
}

/// Mark the quoted tail of sanitized email HTML with [`HTML_QUOTE_ATTR`]: the
/// quote container and the attribution line right above it, or everything from
/// Outlook's header block on. Returns the input untouched when there is nothing
/// to fold. `forward` is whether the mail is known to be a forward, which
/// Outlook's markup -- the same for a reply and a forward -- does not say.
pub fn mark_html_quote(html: &str, forward: bool) -> String {
    find_and_mark_html_quote(html, forward)
        .or_else(|| mark_text_quote(html, forward))
        .unwrap_or_else(|| html.to_string())
}

/// Elements that start a line of their own when they sit among a quote's text.
const BLOCK_ELEMENTS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "div",
    "dl",
    "fieldset",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "ul",
    "caption",
    "center",
    "dd",
    "details",
    "figcaption",
    "hgroup",
    "summary",
    "dt",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
];

/// One line of the document as the reader sees it: what lies between two
/// `<br>`s or block edges, in document order, whatever elements it crosses.
#[derive(Default)]
struct TextLine {
    /// The node the line starts with: its first text, `<br>` or image.
    first: Option<NodeId>,
    text: String,
    content: bool,
    /// Whether an inline element opens or closes inside it that could be laid
    /// out as a block, breaking the line where this reading does not.
    styled: bool,
}

impl TextLine {
    fn quoted(&self) -> bool {
        self.text.trim_start().starts_with('>')
    }

    fn blank(&self) -> bool {
        !self.content
    }

    /// A signature delimiter (a "-- " line) or GitHub's notification footer ("—" then
    /// "Reply to this email directly", on this line or the `next`): what
    /// follows folds with the quote, as in a plain body. A bare em dash is too
    /// common a line start to go by.
    fn signature(&self, next: Option<&TextLine>) -> bool {
        let text = self.text.trim();
        let footer = |text: &str| {
            text.trim_start()
                .starts_with("Reply to this email directly")
        };
        text == "--"
            || text.strip_prefix('—').is_some_and(|rest| {
                footer(rest)
                    || (rest.trim().is_empty() && next.is_some_and(|line| footer(&line.text)))
            })
    }
}

fn text_lines(doc: &Html) -> Vec<TextLine> {
    let is_block = |node: NodeRef<Node>| {
        node.value()
            .as_element()
            .is_some_and(|element| BLOCK_ELEMENTS.contains(&element.name()))
    };
    let mut lines = Vec::new();
    let mut line = TextLine::default();
    // With a stylesheet any element may be laid out as a block; without one,
    // only one with attributes beyond a link's.
    let stylesheet = doc.tree.root().descendants().any(|node| {
        node.value()
            .as_element()
            .is_some_and(|el| el.name() == "style")
    });
    let styled = |node: NodeRef<Node>| {
        node.value().as_element().is_some_and(|element| {
            element.name() != "br"
                && (stylesheet
                    || element
                        .attrs()
                        .any(|(name, _)| !matches!(name, "href" | "rel" | "target" | "title")))
        })
    };
    for edge in doc.tree.root().traverse() {
        // An inline element that may be laid out as a block breaks the line
        // where it opens or closes.
        let (Edge::Open(node) | Edge::Close(node)) = edge;
        if line.first.is_some() && !is_block(node) && styled(node) {
            line.styled = true;
        }
        match edge {
            Edge::Open(node) | Edge::Close(node) if is_block(node) => {
                if line.first.is_some() {
                    lines.push(std::mem::take(&mut line));
                }
            }
            Edge::Open(node) => match node.value() {
                Node::Element(element) if element.name() == "br" => {
                    line.first.get_or_insert(node.id());
                    lines.push(std::mem::take(&mut line));
                }
                Node::Element(_) if is_content(node) => {
                    line.first.get_or_insert(node.id());
                    line.content = true;
                }
                Node::Text(text) if is_content(node) => {
                    line.first.get_or_insert(node.id());
                    line.text.push_str(text);
                    line.content = true;
                }
                // Kept for its newlines, which may break the line (see
                // `mark_text_quote`).
                Node::Text(text) if line.first.is_some() && text.trim().is_empty() => {
                    line.text.push_str(text);
                }
                _ => {}
            },
            Edge::Close(_) => {}
        }
    }
    if line.first.is_some() {
        lines.push(line);
    }
    lines
}

/// Fold a quote written as text: "> " lines between `<br>`s, which is how
/// GitHub and other plain-text-to-HTML converters render a quoted reply. The
/// lines are read across elements, as the reader sees them, and everything
/// from the quote's first line to the end folds: elements carry the marker,
/// bare text is wrapped in a marked `<span>`.
fn mark_text_quote(html: &str, forward: bool) -> Option<String> {
    if forward || !html.contains("&gt;") {
        return None;
    }
    let mut doc = Html::parse_fragment(html);
    // A container or Outlook quote that was left open -- answered inline, or a
    // forward -- stays open, whatever text quotes it holds.
    let containers = Selector::parse(HTML_QUOTE_SELECTOR).ok()?;
    if doc.select(&containers).next().is_some() || outlook_header(&doc, &containers).is_some() {
        return None;
    }
    let lines = text_lines(&doc);
    // A line holding a newline, one side of it quoted, is a quote that a
    // stylesheet, `<pre>` or `white-space` may or may not break there: the
    // reader could see an answer under it that would fold with it. Left alone.
    if lines.iter().any(|line| {
        let mut parts = line.text.lines().filter(|part| !part.trim().is_empty());
        let quoted = line
            .text
            .lines()
            .any(|part| part.trim_start().starts_with('>'));
        quoted && parts.nth(1).is_some()
    }) {
        return None;
    }
    let last_quoted = lines.iter().rposition(TextLine::quoted)?;

    // Only blank lines may follow the quote, up to a signature, under which
    // everything folds with it, as in a plain body.
    for (index, line) in lines.iter().enumerate().skip(last_quoted + 1) {
        if line.signature(lines.get(index + 1)) {
            break;
        }
        if !line.blank() {
            return None;
        }
    }

    // Walk up through the quoted run; blank lines inside it belong to it only
    // when more quoted lines sit above them.
    let mut start = last_quoted;
    let mut cursor = last_quoted;
    while cursor > 0 {
        if lines[cursor - 1].quoted() {
            cursor -= 1;
            start = cursor;
        } else if lines[cursor - 1].blank() {
            cursor -= 1;
        } else {
            break;
        }
    }
    // The line right above the quote (one blank line away at most) heading a
    // forward ("---------- Forwarded message ---------") keeps it open.
    let mut above = start;
    if above > 0 && lines[above - 1].blank() {
        above -= 1;
    }
    if above > 0 && is_forward_header(&lines[above - 1].text) {
        return None;
    }
    let quote: String = lines[start..=last_quoted]
        .iter()
        .flat_map(|line| {
            line.text
                .trim_start_matches(|c: char| c == '>' || c.is_whitespace())
                .chars()
        })
        .take(200)
        .collect();
    if is_forward_header(&quote) {
        return None;
    }
    // The attribution sits just above the quote, at most one blank line away.
    let mut above = start;
    if above > 0 && lines[above - 1].blank() {
        above -= 1;
    }
    if above > 0 && is_attribution(&lines[above - 1].text) {
        let line = above - 1;
        start = line;
        if line > 0
            && !lines[line - 1].blank()
            && is_attribution_continuation(&lines[line - 1].text, &lines[line].text)
        {
            start = line - 1;
        }
        let attribution: String = lines[start..=line]
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        if is_forward_header(&attribution) || attribution.to_lowercase().contains("forward") {
            return None;
        }
    }
    if lines[start..=last_quoted].iter().any(|line| line.styled) {
        return None;
    }
    // The blank lines that set the quote off from the reply fold with it, so
    // the toggle sits right under the reply.
    while start > 0 && lines[start - 1].blank() {
        start -= 1;
    }

    // Start at the outermost element the quote's first line opens, so a
    // wrapper holding only quote folds whole rather than leaving its box.
    let top = doc.root_element().id();
    let mut first = doc.tree.get(lines[start].first?)?;
    while let Some(parent) = first.parent()
        && parent.id() != top
        && first.prev_siblings().all(|sibling| match sibling.value() {
            Node::Text(text) => text.trim().is_empty(),
            Node::Comment(_) => true,
            _ => false,
        })
    {
        first = parent;
    }
    // It and everything after it, up through the elements it sits in.
    let mut folded = vec![first.id()];
    let mut current = first;
    loop {
        folded.extend(current.next_siblings().map(|node| node.id()));
        match current.parent() {
            Some(parent) if parent.id() != top => current = parent,
            _ => break,
        }
    }

    let template = Html::parse_fragment("<span></span>");
    let span = template
        .root_element()
        .first_child()
        .and_then(|node| node.value().as_element())?
        .clone();
    let mut marked = Vec::new();
    for id in folded {
        match doc.tree.get(id)?.value() {
            Node::Element(_) => marked.push(id),
            Node::Text(text) if !text.trim().is_empty() => {
                let wrapper = doc
                    .tree
                    .get_mut(id)?
                    .insert_before(Node::Element(span.clone()))
                    .id();
                doc.tree.get_mut(wrapper)?.append_id(id);
                marked.push(wrapper);
            }
            _ => {}
        }
    }
    mark(doc, marked)
}

fn find_and_mark_html_quote(html: &str, forward: bool) -> Option<String> {
    let container =
        html.contains("gmail_quote") || html.contains("yahoo_quoted") || html.contains("cite");
    let outlook = !forward && (html.contains("divRplyFwdMsg") || html.contains("border-top"));
    if !container && !outlook {
        return None;
    }
    let doc = Html::parse_fragment(html);
    let selector = Selector::parse(HTML_QUOTE_SELECTOR).ok()?;

    // Before the containers: an Outlook reply to a Gmail one has a
    // `gmail_quote` deep in its own quote, which alone would fold only the
    // oldest mail in the thread.
    // Its verdict stands: where it finds the quote but leaves it open -- one
    // answered inline -- that `gmail_quote` may be where the answers are.
    if outlook && let Some(header) = outlook_header(&doc, &selector) {
        let marked = outlook_quote(&doc, header)?;
        return mark(doc, marked);
    }
    if !container {
        return None;
    }

    let quote = doc.select(&selector).find(|el| {
        let outermost = !el
            .ancestors()
            .filter_map(ElementRef::wrap)
            .any(|ancestor| selector.matches(&ancestor));
        outermost && trailing_elements(**el).is_some()
    })?;
    let trailing = trailing_elements(*quote)?;
    if is_forward_header(&quote.text().take(200).collect::<String>()) {
        return None;
    }

    let mut marked = vec![quote.id()];
    let mut top = *quote;
    if let Some(attribution) = attribution_above(*quote) {
        let text: String = attribution.text().collect();
        if is_forward_header(&text) || text.to_lowercase().contains("forward") {
            return None;
        }
        marked.push(attribution.id());
        top = *attribution;
    }
    // The line breaks that set the quote off from the reply fold with it, so
    // the toggle sits right under the reply rather than a blank line below.
    for sibling in top.prev_siblings() {
        match sibling.value() {
            Node::Text(text) if text.trim().is_empty() => continue,
            Node::Element(element) if element.name() == "br" => marked.push(sibling.id()),
            _ => break,
        }
    }
    marked.extend(trailing);
    mark(doc, marked)
}

/// The header block of the mail's own Outlook quote: the first one, unless it
/// is inside a quote container, where it belongs to a mail quoted by another
/// client.
fn outlook_header<'a>(doc: &'a Html, containers: &Selector) -> Option<ElementRef<'a>> {
    let selector = Selector::parse(OUTLOOK_HEADER_SELECTOR).ok()?;
    let header = doc.select(&selector).find(|el| is_outlook_header(*el))?;
    let contained = header
        .ancestors()
        .filter_map(ElementRef::wrap)
        .any(|ancestor| containers.matches(&ancestor));
    (!contained).then_some(header)
}

/// The elements of an Outlook quote: its header block, the rule and blank lines
/// above that, and every element after it. `None` when the quote is not to be
/// folded: the sender answered inside it, or bare text follows the block,
/// which cannot carry the marker.
fn outlook_quote(doc: &Html, header: ElementRef) -> Option<Vec<NodeId>> {
    let mut marked = vec![header.id()];
    // The rule and the blank lines that set the quote off, which the desktop
    // Outlook leaves outside the wrapper it puts around the header block.
    let mut current = *header;
    'above: loop {
        for sibling in current.prev_siblings() {
            if sibling.descendants().any(is_content) {
                break 'above;
            }
            if sibling.value().is_element() {
                marked.push(sibling.id());
            }
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    let mut after = Vec::new();
    let mut current = *header;
    loop {
        for sibling in current.next_siblings() {
            if sibling.value().is_element() {
                after.push(sibling.id());
            } else if is_content(sibling) {
                return None;
            }
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    // Answered inline: the quote is part of the reply. Only Outlook on the
    // web, which is what writes the id, says where the sender typed.
    let typed = Selector::parse(&format!(".{OUTLOOK_TYPED_CLASS}")).ok()?;
    if header.value().id().is_some()
        && doc.select(&typed).any(|el| {
            el.descendants().any(is_content)
                && el
                    .ancestors()
                    .chain(std::iter::once(*el))
                    .any(|node| after.contains(&node.id()))
        })
    {
        return None;
    }
    marked.extend(after);
    Some(marked)
}

/// Whether `element` is the header block Outlook puts over a quoted mail. The
/// id is Outlook's own. The desktop one has only its inline style to go by --
/// `border:none;border-top:solid #E1E1E1 1.0pt;padding:3.0pt 0cm 0cm 0cm`, the
/// color and the unit varying by version and locale -- so it also has to hold
/// the header labels.
fn is_outlook_header(element: ElementRef) -> bool {
    if element
        .value()
        .id()
        .is_some_and(|id| id.ends_with("divRplyFwdMsg"))
    {
        return true;
    }
    let style: String = element
        .value()
        .attr("style")
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    if !style.contains("border-top:solid") || !style.contains("padding:3.0pt0") {
        return false;
    }
    // "From:", "Sent:", "To:", "Subject:" in whatever language, each in bold.
    // One label alone is any note ruled off under a heading.
    let Ok(labels) = Selector::parse("b, strong") else {
        return false;
    };
    element
        .select(&labels)
        .filter(|label| {
            let text: String = label.text().collect();
            text.trim_end().ends_with([':', '：'])
        })
        .count()
        >= 2
}

/// Stamp [`HTML_QUOTE_ATTR`] on the `marked` elements and serialize the
/// document, unless that would fold away everything the reader sees.
fn mark(mut doc: Html, marked: Vec<NodeId>) -> Option<String> {
    let folded_away = |node: NodeRef<Node>| {
        node.ancestors()
            .chain(std::iter::once(node))
            .any(|n| marked.contains(&n.id()))
    };
    if !doc
        .tree
        .root()
        .descendants()
        .any(|node| is_content(node) && !folded_away(node))
    {
        return None;
    }

    // Take the attribute's name off a parsed one rather than building a
    // `QualName` by hand.
    let template = Html::parse_fragment(&format!(r#"<i {HTML_QUOTE_ATTR}=""></i>"#));
    let name = template
        .root_element()
        .first_child()
        .and_then(|node| node.value().as_element())
        .and_then(|element| element.attrs.first())
        .map(|(name, _)| name.clone())?;
    for id in marked {
        if let Some(mut node) = doc.tree.get_mut(id)
            && let Node::Element(element) = node.value()
            && !element
                .attrs
                .iter()
                .any(|(attr, _)| attr.local == name.local)
        {
            element.attrs.extend([(name.clone(), Default::default())]);
        }
    }
    Some(doc.root_element().inner_html())
}

/// Whether nothing the reader would see follows `node` in document order but a
/// signature, which Gmail puts under the quote. Returns the elements after it —
/// the signature and the empty lines around it (`<div><br></div>`, `<br>`) — to
/// fold along with the quote, since left behind they are only blank space under
/// the toggle; `None` when other content follows.
fn trailing_elements(node: NodeRef<Node>) -> Option<Vec<NodeId>> {
    let mut elements = Vec::new();
    let mut current = node;
    loop {
        for sibling in current.next_siblings() {
            if is_signature(sibling) || !sibling.descendants().any(is_content) {
                if sibling.value().is_element() {
                    elements.push(sibling.id());
                }
            } else {
                return None;
            }
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return Some(elements),
        }
    }
}

/// Gmail's signature and its "-- " prefix, or Thunderbird's signature.
fn is_signature(node: NodeRef<Node>) -> bool {
    node.value().as_element().is_some_and(|element| {
        element.classes().any(|class| {
            matches!(
                class,
                "gmail_signature" | "gmail_signature_prefix" | "moz-signature"
            )
        })
    })
}

/// The element holding the attribution line just above a quote: its previous
/// element sibling, past line breaks and whitespace, when its text reads as one.
fn attribution_above(quote: NodeRef<Node>) -> Option<ElementRef> {
    for sibling in quote.prev_siblings() {
        match sibling.value() {
            Node::Text(text) if text.trim().is_empty() => continue,
            Node::Comment(_) => continue,
            Node::Element(element) if element.name() == "br" => continue,
            Node::Element(_) => {
                let element = ElementRef::wrap(sibling)?;
                let text: String = element.text().collect();
                return is_attribution(&text).then_some(element);
            }
            _ => return None,
        }
    }
    None
}

/// A node the reader sees: text outside `<style>`, or an image or video.
fn is_content(node: NodeRef<Node>) -> bool {
    match node.value() {
        Node::Text(text) => {
            !text
                .chars()
                .all(|c| c.is_whitespace() || c == '\u{200b}' || c == '\u{feff}')
                && !node
                    .parent()
                    .and_then(|parent| parent.value().as_element().map(|el| el.name() == "style"))
                    .unwrap_or(false)
        }
        Node::Element(element) => matches!(element.name(), "img" | "video"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_folds_a_trailing_quote_with_its_attribution() {
        let body = "Sounds good, see you then.\n\nOn Mon, Sep 1, 2025 at 9:00 AM Jane <jane@example.com> wrote:\n> Lunch on Friday?\n>\n> Jane\n";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(
            &body[start..],
            "On Mon, Sep 1, 2025 at 9:00 AM Jane <jane@example.com> wrote:\n> Lunch on Friday?\n>\n> Jane\n"
        );
    }

    #[test]
    fn plain_takes_a_wrapped_attribution_whole() {
        let body = "Yes.\n\nOn Mon, Sep 1, 2025 at 9:00 AM Jane Doe <\njane@example.com> wrote:\n\n> Lunch?\n";
        let start = plain_quote_start(body).expect("quote");
        assert!(body[start..].starts_with("On Mon"), "{:?}", &body[start..]);
    }

    #[test]
    fn plain_folds_a_signature_under_the_quote_with_it() {
        let body = "Does Tuesday work?\n\nOn Tue, Sep 8, 2026 at 8:14 PM Alex <alex@example.com>\nwrote:\n\n> Both days are good.\n>\n\n\n-- \n\n*MS*\n";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(&body[..start], "Does Tuesday work?\n\n");
    }

    #[test]
    fn plain_keeps_a_signature_above_the_quote_out_of_it() {
        let body = "Does Tuesday work?\n-- \nMS\n\nOn Mon, Ping wrote:\n> Both days are good.";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(&body[..start], "Does Tuesday work?\n-- \nMS\n\n");
        // A signature with no quote anywhere is nothing to fold.
        assert_eq!(plain_quote_start("Hi\n-- \nMS"), None);
    }

    #[test]
    fn plain_offset_counts_utf16_units() {
        // "é" is one UTF-16 unit but two bytes; "😀" is two units and four bytes.
        let body = "é😀\n\n> quoted";
        assert_eq!(
            plain_quote_start(body),
            Some("é😀\n\n".encode_utf16().count())
        );
    }

    #[test]
    fn plain_leaves_inline_replies_alone() {
        let body = "> Lunch on Friday?\nYes!\n> Where?\nThe usual place.";
        assert_eq!(plain_quote_start(body), None);
    }

    #[test]
    fn plain_only_folds_the_trailing_quote_of_an_inline_reply() {
        let body = "> Lunch?\nYes!\n\n> older history\n> more";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(&body[start..], "> older history\n> more");
    }

    #[test]
    fn plain_keeps_a_body_that_is_only_a_quote() {
        assert_eq!(plain_quote_start("On Mon Jane wrote:\n> hi"), None);
        assert_eq!(plain_quote_start("> hi\n> there"), None);
        assert_eq!(plain_quote_start("no quote here"), None);
    }

    #[test]
    fn plain_takes_an_attribution_wrapped_after_the_address() {
        let body = "Yes.\n\nOn Mon, Sep 1, 2025 at 9:00 AM Jane Doe <jane@example.com>\nwrote:\n> Lunch?\n";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(&body[..start], "Yes.\n\n");
    }

    #[test]
    fn plain_takes_an_attribution_wrapped_after_the_date() {
        let body = "Works now, thanks!\nJane Doe <jane@example.com>, 29 Eyl 2026 Sal, 14:46 tarihinde şunu\nyazdı:\n> Fixed in 1.2.3\n";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(&body[..start], "Works now, thanks!\n");
    }

    #[test]
    fn plain_keeps_a_line_with_an_address_above_a_short_attribution() {
        let body = "Hello\nPlease contact <help@example.com> for option 2.\nJane wrote:\n> Which option?\n";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(&body[start..], "Jane wrote:\n> Which option?\n");
    }

    #[test]
    fn plain_keeps_a_line_with_an_address_and_a_time_above_a_short_attribution() {
        for line in [
            "Please email <help@example.com> at 14:00.",
            "Please email <help@example.com>, from 9:00 to 17:00.",
        ] {
            let body = format!("Confirmed.\n{line}\nJane wrote:\n> Who should I contact?\n");
            let start = plain_quote_start(&body).expect("quote");
            assert_eq!(
                &body[start..],
                "Jane wrote:\n> Who should I contact?\n",
                "{line}"
            );
        }
    }

    #[test]
    fn plain_keeps_the_reply_above_a_short_attribution() {
        for attribution in [
            "Jane wrote:",
            "<jane@example.com> wrote:",
            "jane@example.com wrote:",
        ] {
            let body = format!("Hi Bob,\n\nPlease proceed.\n{attribution}\n> Is this approved?");
            let start = plain_quote_start(&body).expect("quote");
            assert_eq!(
                &body[..start],
                "Hi Bob,\n\nPlease proceed.\n",
                "{attribution}"
            );
        }
    }

    #[test]
    fn plain_does_not_fold_the_line_before_an_unwrapped_attribution() {
        let body = "Thanks\nOn Mon, Sep 1, 2025 at 9:00 AM Jane <jane@example.com> wrote:\n> hi";
        let start = plain_quote_start(body).expect("quote");
        assert_eq!(&body[..start], "Thanks\n");
    }

    #[test]
    fn plain_leaves_forwards_alone() {
        let body = "FYI\n\n-----Original Message-----:\n> the content";
        assert_eq!(plain_quote_start(body), None);
    }

    #[test]
    fn html_marks_gmail_quote() {
        let html = r#"<div dir="ltr">Sounds good</div><br><div class="gmail_quote"><div class="gmail_attr">On Mon, Jane wrote:<br></div><blockquote class="gmail_quote">Lunch?</blockquote></div>"#;
        let out = mark_html_quote(html, false);
        assert!(
            out.contains(r#"<div class="gmail_quote" data-meron-quote="">"#),
            "{out}"
        );
        // Only the outermost container is marked, not the blockquote inside it.
        assert!(out.contains(r#"<blockquote class="gmail_quote">"#), "{out}");
        // The <br> setting the quote off from the reply folds with it.
        assert!(
            out.contains(r#"</div><br data-meron-quote=""><div"#),
            "{out}"
        );
        assert!(out.contains("Sounds good"));
    }

    #[test]
    fn html_folds_a_gmail_signature_under_the_quote() {
        let html = r#"<div dir="ltr">Does Tuesday work?</div><br><div class="gmail_quote gmail_quote_container"><div class="gmail_attr">On Tue, Ping wrote:<br></div><blockquote class="gmail_quote"><p>Both days are good.</p></blockquote></div><div><br clear="all"></div><div><br></div><span class="gmail_signature_prefix">-- </span><br><div dir="ltr" class="gmail_signature"><b>MS</b></div>"#;
        let out = mark_html_quote(html, false);
        // The quote, the <br> above it, the two empty divs, the "-- " prefix,
        // the <br> after it and the signature: nothing blank is left behind.
        assert_eq!(out.matches(HTML_QUOTE_ATTR).count(), 7, "{out}");
        assert!(
            out.starts_with(r#"<div dir="ltr">Does Tuesday work?</div><br data-meron-quote="">"#),
            "{out}"
        );
        assert!(out.contains(r#"data-meron-quote=""><b>MS</b>"#), "{out}");
        // Content that isn't a signature after the quote still keeps it open.
        let after = r#"<p>Yes</p><div class="gmail_quote">Lunch?</div><p>P.S. bring cake</p>"#;
        assert_eq!(mark_html_quote(after, false), after);
        // A quote and a signature with no reply is nothing to fold.
        let bare = r#"<div class="gmail_quote">Lunch?</div><div class="gmail_signature">MS</div>"#;
        assert_eq!(mark_html_quote(bare, false), bare);
    }

    #[test]
    fn html_marks_apple_and_thunderbird_attribution() {
        let html = r#"<p>Yes!</p><div class="moz-cite-prefix">On 9/1/25 Jane wrote:</div><blockquote type="cite"><p>Lunch?</p></blockquote>"#;
        let out = mark_html_quote(html, false);
        assert!(
            out.contains(r#"<div class="moz-cite-prefix" data-meron-quote="">"#),
            "{out}"
        );
        assert!(
            out.contains(r#"<blockquote type="cite" data-meron-quote="">"#),
            "{out}"
        );
    }

    #[test]
    fn html_leaves_inline_and_quote_only_bodies_alone() {
        let inline = r#"<blockquote type="cite">Lunch?</blockquote><p>Yes!</p>"#;
        assert_eq!(mark_html_quote(inline, false), inline);
        // Nor is a Gmail quote inside it folded instead, answers and all.
        let nested = r#"<div class="elementToProof">See below</div><div id="divRplyFwdMsg"><b>From:</b> Jane</div><div><div>Lunch?</div><div class="gmail_quote">On Mon, Joe wrote:<blockquote>Hungry?</blockquote><div class="elementToProof">Yes, at noon</div></div></div>"#;
        assert_eq!(mark_html_quote(nested, false), nested);
        let only = r#"<div class="gmail_quote">Lunch?</div>"#;
        assert_eq!(mark_html_quote(only, false), only);
        let plain = "<p>no quote</p>";
        assert_eq!(mark_html_quote(plain, false), plain);
    }

    #[test]
    fn html_ignores_trailing_whitespace_and_styles() {
        let html = r#"<p>Yes!</p><div class="gmail_quote">Lunch?</div><div><br></div><style>p{color:red}</style>"#;
        assert!(mark_html_quote(html, false).contains(HTML_QUOTE_ATTR));
    }

    #[test]
    fn html_marks_outlook_web_quote() {
        // Outlook on the web: no container, the quote is the header block's
        // siblings, and an older quote inside it repeats the ids prefixed.
        let html = r#"<div class="elementToProof">t2</div><div id="appendonsend"></div><hr style="display:inline-block;width:98%"><div id="divRplyFwdMsg" dir="ltr"><font><b>From:</b> Jane &lt;jane@example.com&gt;<br><b>Subject:</b> Fw: Lunch</font><div>&nbsp;</div></div><style>p{margin:0}</style><div dir="ltr"><div class="x_elementToProof">t1</div><div id="x_appendonsend"></div><hr><div id="x_divRplyFwdMsg"><b>From:</b> Joe</div><div>Lunch?</div></div>"#;
        let out = mark_html_quote(html, false);
        assert!(
            out.contains(r#"<div class="elementToProof">t2</div>"#),
            "{out}"
        );
        for folded in [
            r#"<div id="appendonsend" data-meron-quote="">"#,
            r#"<hr style="display:inline-block;width:98%" data-meron-quote="">"#,
            r#"<div dir="ltr" id="divRplyFwdMsg" data-meron-quote="">"#,
            r#"<div dir="ltr" data-meron-quote="">"#,
        ] {
            assert!(out.contains(folded), "{folded}: {out}");
        }
        // Only the outermost quote is marked.
        assert!(out.contains(r#"<div id="x_divRplyFwdMsg">"#), "{out}");
        // The markup is the same for a forward: only the subject tells.
        assert_eq!(mark_html_quote(html, true), html);
    }

    #[test]
    fn html_marks_outlook_desktop_quote() {
        let html = r#"<div class="WordSection1"><p class="MsoNormal">Bonjour</p><p class="MsoNormal">&nbsp;</p><div><div style="border:none;border-top:solid #E1E1E1 1.0pt;padding:3.0pt 0cm 0cm 0cm"><p class="MsoNormal"><b>De :</b> Jane<br><b>Objet :</b> Devis</p></div></div><p class="MsoNormal">Le devis</p></div>"#;
        let out = mark_html_quote(html, false);
        assert!(out.contains(r#"<p class="MsoNormal">Bonjour</p>"#), "{out}");
        assert!(
            out.contains(r#"<p class="MsoNormal" data-meron-quote="">&nbsp;</p>"#),
            "{out}"
        );
        assert!(
            out.contains(r#"padding:3.0pt 0cm 0cm 0cm" data-meron-quote="">"#),
            "{out}"
        );
        assert!(
            out.contains(r#"<p class="MsoNormal" data-meron-quote="">Le devis</p>"#),
            "{out}"
        );
    }

    #[test]
    fn html_leaves_other_outlook_shapes_alone() {
        // Nothing above the header block: the quote is the whole message.
        let only = r#"<div id="divRplyFwdMsg"><b>From:</b> Jane</div><div>Lunch?</div>"#;
        assert_eq!(mark_html_quote(only, false), only);
        // A top border that rules off something other than mail headers.
        for note in [
            "Thanks, and see you there",
            "<b>Note:</b> payment is due Friday",
        ] {
            let ruled = format!(
                r#"<p>Hi</p><div style="border-top:solid #ccc 1.0pt;padding:3.0pt 0cm 0cm 0cm">{note}</div><p>Jane</p>"#
            );
            assert_eq!(mark_html_quote(&ruled, false), ruled);
        }
        // An answer typed into the quote, which Outlook on the web marks: the
        // quote is then part of the reply.
        let inline = r#"<div class="elementToProof">See below</div><div id="divRplyFwdMsg"><b>From:</b> Jane</div><div><div class="x_elementToProof">Lunch?</div><div class="elementToProof">Yes, at noon</div></div>"#;
        assert_eq!(mark_html_quote(inline, false), inline);
        // Bare text after the block cannot be marked, so nothing is.
        let bare = r#"<p>Yes</p><div id="divRplyFwdMsg"><b>From:</b> Jane</div>Lunch?"#;
        assert_eq!(mark_html_quote(bare, false), bare);
    }

    #[test]
    fn html_folds_the_whole_outlook_quote_over_a_gmail_one_in_it() {
        let html = r#"<div>Yes</div><div id="divRplyFwdMsg"><b>From:</b> Jane</div><div><div>Lunch?</div><div class="gmail_quote">On Mon, Joe wrote:<blockquote>Hungry</blockquote></div></div>"#;
        let out = mark_html_quote(html, false);
        assert!(
            out.contains(r#"<div id="divRplyFwdMsg" data-meron-quote="">"#),
            "{out}"
        );
        assert!(out.contains(r#"<div class="gmail_quote">"#), "{out}");
        // Quoted by Gmail in turn, the Outlook block is inside the container.
        let nested = r#"<div>Yes</div><div class="gmail_quote">On Mon, Jane wrote:<blockquote><div>t1</div><div id="divRplyFwdMsg"><b>From:</b> Joe</div><div>Lunch?</div></blockquote></div>"#;
        let out = mark_html_quote(nested, false);
        assert!(
            out.contains(r#"<div class="gmail_quote" data-meron-quote="">"#),
            "{out}"
        );
        assert!(out.contains(r#"<div id="divRplyFwdMsg">"#), "{out}");
    }

    #[test]
    fn forward_subjects() {
        for subject in [
            "Fw: Lunch",
            "FWD: Lunch",
            "TR : Devis",
            "WG: Angebot",
            "转发：午饭",
        ] {
            assert!(is_forward_subject(subject), "{subject}");
        }
        for subject in ["Re: Fw: Lunch", "Lunch", "Forwarding address: new", ""] {
            assert!(!is_forward_subject(subject), "{subject}");
        }
    }

    #[test]
    fn html_folds_a_text_quote_and_the_footer_under_it() {
        // GitHub renders an emailed reply's quote as "> " text between <br>s.
        let html = "<div><strong>jane</strong> left a comment</div>\nStill not working\n<br>\n\n<br>\nRegards\n<br>\nJane\n<br>\n\n<br>\nOn Mon, 16 Nov, 2026, Joe Doe, ***@***.***&gt; wrote:\n<br>\n\n<br>\n&gt; please try 1.2.3\n<br>\n&gt;\n<br>\n&gt; Reply on GitHub\n<br>\n\n\n<p style=\"color:#666\">&mdash;<br>Reply to this email directly</p>";
        let out = mark_html_quote(html, false);
        let (reply, quote) = out.split_once(r#"<br data-meron-quote="">"#).expect(&out);
        assert!(reply.ends_with("Jane\n<br>\n\n"), "{out}");
        assert!(!quote.contains("<br>\n"), "every line folds: {out}");
        assert!(
            quote.contains("<span data-meron-quote=\"\">\nOn Mon"),
            "{out}"
        );
        assert!(
            quote.ends_with(
                r#"<p style="color:#666" data-meron-quote="">—<br>Reply to this email directly</p>"#
            ),
            "{out}"
        );
    }

    #[test]
    fn html_folds_a_text_quote_a_paragraph_per_line() {
        let html = "<p>Reply</p><p>On Mon, Jane wrote:</p><p>&gt; first</p><p>&gt; second</p>";
        let out = mark_html_quote(html, false);
        assert_eq!(
            out,
            r#"<p>Reply</p><p data-meron-quote="">On Mon, Jane wrote:</p><p data-meron-quote="">&gt; first</p><p data-meron-quote="">&gt; second</p>"#
        );
        let only = "<p>&gt; first</p><p>&gt; second</p>";
        assert_eq!(mark_html_quote(only, false), only);
        // Quote lines in inline wrappers fold as one run too.
        let spans = "Reply<br><span>&gt; first</span><br><span>&gt; second</span>";
        assert_eq!(
            mark_html_quote(spans, false),
            r#"Reply<br><span data-meron-quote="">&gt; first</span><br data-meron-quote=""><span data-meron-quote="">&gt; second</span>"#
        );
        let only = "<span>&gt; first</span><br><span>&gt; second</span>";
        assert_eq!(mark_html_quote(only, false), only);
    }

    #[test]
    fn html_leaves_inline_text_quotes_alone() {
        // An answer inside a block that starts with a quoted line stays open.
        let answered = "Hi<br><div>&gt; question<br>My answer</div>&gt; older<br>";
        let out = mark_html_quote(answered, false);
        assert!(
            out.starts_with(
                r#"Hi<br><div>&gt; question<br>My answer</div><span data-meron-quote="">"#
            ),
            "{out}"
        );
        // So does one inside an inline wrapper.
        let wrapped = "<p>Reply</p><p><span>&gt; question<br>My answer</span></p><p>&gt; older</p>";
        let out = mark_html_quote(wrapped, false);
        assert!(
            out.contains("My answer</span></p><p data-meron-quote="),
            "{out}"
        );
        // Text quotes inside a container quote left open (a forward) stay open.
        // Nor are answers in preformatted text, in a table, or under a line
        // that only starts like a signature delimiter.
        for html in [
            "<p>Reply</p><pre>&gt; question\nMy answer</pre>",
            "<p>Reply</p><center>&gt; question</center>My answer",
            r#"<p>Reply</p><p>&gt; question <span style="display:block">My answer</span></p>"#,
            r#"<p>Reply</p><p>&gt; question <span class="answer">My answer</span></p>"#,
            r#"<p>Reply</p><span style="display:block">&gt; question</span>My answer"#,
            r#"<style>#answer{display:block}</style><p>Reply</p><p>&gt; question<span id="answer">My answer</span></p>"#,
            r#"<style>b{display:block}</style><p>Reply</p><p>&gt; question<b>My answer</b></p>"#,
            "<p>Reply</p><pre>&gt; question\n<strong>My answer</strong></pre>",
            "<p>Reply</p><pre>&gt; question<i></i>\n<strong>My answer</strong></pre>",
            "<p>Reply</p><p style=\"white-space: break-spaces\">&gt; question\nMy answer</p>",
            "<p>Reply</p><style>.body { white-space: pre-wrap }</style><div class=\"body\">&gt; Question?\nMy answer</div>",
            "<p>Reply</p><table><tr><td>&gt; question</td></tr><tr><td>My answer</td></tr></table>",
            "<p>Reply</p><p>&gt; question</p><p>-- additional clarification</p><p>My answer</p>",
        ] {
            assert_eq!(mark_html_quote(html, false), html);
        }
        let forward = r#"<p>FYI</p><div class="gmail_quote">---------- Forwarded message ---------<br>&gt; Content</div>"#;
        assert_eq!(mark_html_quote(forward, false), forward);
        let forward = "<p>FYI</p><p>\n&gt; -----Original Message-----</p><p>\n&gt; Content</p>";
        assert_eq!(mark_html_quote(forward, false), forward);
        let forward = "FYI<br>---------- Forwarded message ---------<br>&gt; Hello";
        assert_eq!(mark_html_quote(forward, false), forward);
        let inline = "Hi<br>&gt; Lunch?<br>Yes, at noon<br>";
        assert_eq!(mark_html_quote(inline, false), inline);
        let only = "&gt; Lunch?<br>&gt; Jane<br>";
        assert_eq!(mark_html_quote(only, false), only);
        let forward = "FYI<br>&gt; Lunch?<br>";
        assert_eq!(mark_html_quote(forward, true), forward);
    }

    #[test]
    fn html_leaves_forwards_alone() {
        let gmail = r#"<p>FYI</p><div class="gmail_quote"><div class="gmail_attr">---------- Forwarded message ---------<br>From: Jane</div>Content</div>"#;
        assert_eq!(mark_html_quote(gmail, false), gmail);
        let apple = r#"<p>FYI</p><div>Begin forwarded message:</div><blockquote type="cite">Content</blockquote>"#;
        assert_eq!(mark_html_quote(apple, false), apple);
    }
}
