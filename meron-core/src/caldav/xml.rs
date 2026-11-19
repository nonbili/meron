//! Just enough XML for WebDAV: a namespace-resolved element tree, and the
//! `multistatus` (RFC 4918 §13) every PROPFIND and REPORT answers with.
//!
//! Servers disagree on prefixes (`D:`, `d:`, none at all) but not on
//! namespaces, so elements are matched by `(namespace, local name)` and the
//! prefix is thrown away.

use anyhow::{Context, Result};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

pub const DAV: &str = "DAV:";
pub const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
pub const CARDDAV: &str = "urn:ietf:params:xml:ns:carddav";
pub const CALSERVER: &str = "http://calendarserver.org/ns/";
pub const APPLE: &str = "http://apple.com/ns/ical/";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Element {
    pub ns: String,
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    pub fn is(&self, ns: &str, name: &str) -> bool {
        self.ns == ns && self.name == name
    }

    pub fn child(&self, ns: &str, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.is(ns, name))
    }

    pub fn children_named<'a>(
        &'a self,
        ns: &'a str,
        name: &'a str,
    ) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.is(ns, name))
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Trimmed text of this element.
    pub fn text(&self) -> &str {
        self.text.trim()
    }

    /// The text of the first `DAV:href` inside, as properties like
    /// `current-user-principal` and `calendar-home-set` carry their value.
    pub fn href(&self) -> Option<&str> {
        self.child(DAV, "href")
            .map(Element::text)
            .filter(|h| !h.is_empty())
    }
}

pub fn parse(body: &str) -> Result<Element> {
    let mut reader = NsReader::from_str(body);
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    loop {
        let (ns, event) = reader
            .read_resolved_event()
            .context("malformed XML from server")?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let self_closing = matches!(event, Event::Empty(_));
                let mut element = Element {
                    ns: match ns {
                        ResolveResult::Bound(n) => String::from_utf8_lossy(n.as_ref()).into_owned(),
                        _ => String::new(),
                    },
                    name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                    ..Default::default()
                };
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .map(|v| v.into_owned())
                        .unwrap_or_default();
                    element.attrs.push((key, value));
                }
                // A self-closing element has no matching `End`; finish it now.
                if self_closing {
                    close(&mut stack, &mut root, element);
                } else {
                    stack.push(element);
                }
            }
            Event::End(_) => {
                if let Some(element) = stack.pop() {
                    close(&mut stack, &mut root, element);
                }
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.unescape().context("bad XML text")?);
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t.into_inner()));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.context("empty XML document")
}

fn close(stack: &mut [Element], root: &mut Option<Element>, element: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(element),
        None => *root = Some(element),
    }
}

/// One `DAV:response` of a multistatus.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Response {
    /// As the server wrote it: usually an absolute path, percent-encoded.
    pub href: String,
    /// The response-level status, which `sync-collection` uses to say a
    /// member was removed (404). `None` when the response carries propstats.
    pub status: Option<u16>,
    /// Properties the server returned with a 2xx status.
    pub props: Vec<Element>,
}

impl Response {
    pub fn prop(&self, ns: &str, name: &str) -> Option<&Element> {
        self.props.iter().find(|p| p.is(ns, name))
    }

    pub fn prop_text(&self, ns: &str, name: &str) -> Option<&str> {
        self.prop(ns, name)
            .map(Element::text)
            .filter(|t| !t.is_empty())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Multistatus {
    pub responses: Vec<Response>,
    /// The new token a `sync-collection` REPORT ends with.
    pub sync_token: Option<String>,
}

/// The numeric code of a `DAV:status` line such as `HTTP/1.1 404 Not Found`.
pub fn status_code(line: &str) -> Option<u16> {
    line.split_whitespace().nth(1)?.parse().ok()
}

pub fn parse_multistatus(body: &str) -> Result<Multistatus> {
    let root = parse(body)?;
    anyhow::ensure!(
        root.is(DAV, "multistatus"),
        "expected a DAV multistatus, got <{}>",
        root.name
    );
    let mut out = Multistatus {
        sync_token: root
            .child(DAV, "sync-token")
            .map(|t| t.text().to_string())
            .filter(|t| !t.is_empty()),
        ..Default::default()
    };
    for response in root.children_named(DAV, "response") {
        let Some(href) = response.href() else {
            continue;
        };
        let mut parsed = Response {
            href: href.to_string(),
            status: response
                .child(DAV, "status")
                .and_then(|s| status_code(s.text())),
            props: Vec::new(),
        };
        for propstat in response.children_named(DAV, "propstat") {
            let ok = propstat
                .child(DAV, "status")
                .and_then(|s| status_code(s.text()))
                .is_some_and(|code| (200..300).contains(&code));
            if let (true, Some(prop)) = (ok, propstat.child(DAV, "prop")) {
                parsed.props.extend(prop.children.iter().cloned());
            }
        }
        out.responses.push(parsed);
    }
    Ok(out)
}

/// Escape text for an XML body this module writes.
pub fn escape(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}
