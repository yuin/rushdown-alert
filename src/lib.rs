#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt;
use core::fmt::Write;
use core::marker::PhantomData;

use rushdown::{
    as_extension_data, as_kind_data, as_type_data, as_type_data_mut,
    ast::{
        pp_indent, walk, Arena, InlineChars, KindData, NodeKind, NodeRef, NodeType, PrettyPrint,
        TextQualifier, WalkStatus,
    },
    matches_kind,
    parser::{
        AnyAstTransformer, AstTransformer, Context, Parser, ParserExtension, ParserExtensionFn,
        ParserOptions,
    },
    renderer::{
        self,
        html::{self, renderer_extension, RendererExtension, TextWriter},
        BoxRenderNode, NodeRenderer, NodeRendererRegistry, RenderNode, RendererOptions, TextWrite,
    },
    text::{self, Reader},
    util, Result,
};

// AST {{{

/// An alert block (GitHub style `> [!NOTE]` blockquote).
#[derive(Debug)]
pub struct Alert {
    kind: text::Value,
    title: text::Value,
}

impl Alert {
    /// Creates a new [`Alert`] with the given kind.
    ///
    /// The kind is kept as it appears in the source; it is not normalized.
    pub fn new(kind: impl Into<text::Value>) -> Self {
        Self {
            kind: kind.into(),
            title: text::Value::Empty,
        }
    }

    /// Sets the title of this alert.
    pub fn with_title(mut self, title: impl Into<text::Value>) -> Self {
        self.title = title.into();
        self
    }

    /// Returns the kind of this alert as it appears in the source.
    #[inline(always)]
    pub fn kind(&self) -> &text::Value {
        &self.kind
    }

    /// Returns the title of this alert. This is [`text::Value::Empty`] if the alert has no title.
    #[inline(always)]
    pub fn title(&self) -> &text::Value {
        &self.title
    }

    /// Sets the title of this alert.
    pub fn set_title(&mut self, title: impl Into<text::Value>) {
        self.title = title.into();
    }

    /// Returns the title to display. If this alert has no title, a capitalized kind is returned.
    pub fn display_title<'a>(&'a self, source: &'a str) -> Cow<'a, str> {
        let title = self.title.value(source);
        if title.is_empty() {
            Cow::Owned(capitalize(&self.kind.value(source)))
        } else {
            title
        }
    }
}

impl NodeKind for Alert {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "Alert"
    }
}

impl InlineChars for Alert {}

impl PrettyPrint for Alert {
    fn pretty_print(&self, w: &mut dyn Write, source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}kind: {:?}", pp_indent(level), self.kind.value(source))?;
        writeln!(
            w,
            "{}title: {:?}",
            pp_indent(level),
            self.title.value(source)
        )
    }
}

impl From<Alert> for KindData {
    fn from(a: Alert) -> Self {
        KindData::Extension(Box::new(a))
    }
}

// }}} AST

// Parser {{{

/// Kinds of alerts defined by GitHub Flavored Markdown.
pub const GITHUB_ALERT_KINDS: &[&str] = &["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"];

/// Options for the alert parser.
#[derive(Debug, Clone, Copy)]
pub struct AlertParserOptions {
    /// Whether to allow alerts nested in other alerts. GitHub does not allow nested alerts.
    pub allows_nested: bool,

    /// Kinds of alerts to enable. Kind matching is case insensitive.
    pub kinds: &'static [&'static str],
}

impl Default for AlertParserOptions {
    fn default() -> Self {
        Self {
            allows_nested: false,
            kinds: GITHUB_ALERT_KINDS,
        }
    }
}

impl ParserOptions for AlertParserOptions {}

#[derive(Debug)]
struct AlertAstTransformer {
    options: AlertParserOptions,
}

impl AlertAstTransformer {
    fn with_options(options: AlertParserOptions) -> Self {
        Self { options }
    }

    fn transform_blockquote(
        &self,
        arena: &mut Arena,
        blockquote: NodeRef,
        source: &str,
        decoder: text::Decoder,
    ) {
        let Some(paragraph) = arena[blockquote].first_child() else {
            return;
        };
        if !matches_kind!(arena[paragraph], Paragraph) {
            return;
        }
        let Some(first_child) = arena[paragraph].first_child() else {
            return;
        };
        if !matches_kind!(arena[first_child], Text) {
            return;
        }
        if !starts_with_alert_marker(arena, first_child, source) {
            return;
        }

        let mut first_line = String::new();
        let mut remove = Vec::new();
        let mut current = Some(first_child);
        while let Some(node) = current {
            current = arena[node].next_sibling();
            if !matches_kind!(arena[node], Text) {
                continue;
            }
            let text_node = as_kind_data!(arena, node, Text);
            first_line.push_str(&text_node.value().str(source));
            remove.push(node);
            if text_node.has_qualifiers(TextQualifier::SOFT_LINE_BREAK) {
                break;
            }
        }

        let line = &first_line[util::trim_left_space_length(first_line.as_bytes())..];
        let Some(rest) = line.strip_prefix("[!") else {
            return;
        };
        let Some(close) = rest.find(']') else {
            return;
        };
        let kind = &rest[..close];
        if !self
            .options
            .kinds
            .iter()
            .any(|k| k.eq_ignore_ascii_case(kind))
        {
            return;
        }
        let title = &rest[close + 1..];
        let title = &title[util::trim_left_space_length(title.as_bytes())..];

        let mut data = Alert::new(text::Value::from_string(kind, decoder));
        if !title.is_empty() {
            data.set_title(text::Value::from_string(title, decoder));
        }
        let alert = arena.new_node(data);

        for node in remove {
            node.remove(arena);
        }
        if arena[paragraph].first_child().is_none() {
            paragraph.remove(arena);
        }

        let mut children = Vec::new();
        let mut current = arena[blockquote].first_child();
        while let Some(node) = current {
            current = arena[node].next_sibling();
            children.push(node);
        }
        for child in children {
            alert.append_child(arena, child);
        }

        if let Some(pos) = arena[blockquote].pos() {
            arena[alert].set_pos(pos);
        }
        let blank = as_type_data!(arena, blockquote, Block).has_blank_previous_line();
        as_type_data_mut!(arena, alert, Block).set_blank_previous_line(blank);
        if let Some(parent) = arena[blockquote].parent() {
            parent.replace_child(arena, blockquote, alert);
        }
    }
}

impl AstTransformer for AlertAstTransformer {
    fn transform(
        &self,
        arena: &mut Arena,
        doc_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut Context,
    ) {
        let mut blockquotes = Vec::new();
        let _ = walk(arena, doc_ref, &mut |arena: &Arena,
                                           node_ref: NodeRef,
                                           entering: bool|
         -> Result<WalkStatus> {
            if entering
                && matches_kind!(arena[node_ref], Blockquote)
                && (self.options.allows_nested || arena[node_ref].parent() == Some(doc_ref))
            {
                blockquotes.push(node_ref);
            }
            Ok(WalkStatus::Continue)
        });
        let source = reader.source();
        let decoder = reader.decoder();
        for blockquote in blockquotes {
            self.transform_blockquote(arena, blockquote, source, decoder);
        }
    }
}

impl From<AlertAstTransformer> for AnyAstTransformer {
    fn from(t: AlertAstTransformer) -> Self {
        AnyAstTransformer::Extension(Box::new(t))
    }
}

fn starts_with_alert_marker(arena: &Arena, first: NodeRef, source: &str) -> bool {
    let marker = b"[!";
    let mut matched = 0;
    let mut current = Some(first);
    while let Some(node) = current {
        current = arena[node].next_sibling();
        if !matches_kind!(arena[node], Text) {
            continue;
        }
        let text = as_kind_data!(arena, node, Text);
        for &byte in text.value().bytes(source).iter() {
            if matched == 0 && util::is_space(byte) {
                continue;
            }
            if byte != marker[matched] {
                return false;
            }
            matched += 1;
            if matched == marker.len() {
                return true;
            }
        }
        if text.has_qualifiers(TextQualifier::SOFT_LINE_BREAK) {
            break;
        }
    }
    false
}

// }}} Parser

// Renderer {{{

const NOTE_ICON: &str = r#"<svg
  class="markdown-icon markdown-icon-info mr-2"
  viewBox="0 0 16 16"
  width="16"
  height="16"
  aria-hidden="true"
>
  <path d="M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM6.5 7.75A.75.75 0 0 1 7.25 7h1a.75.75 0 0 1 .75.75v2.75h.25a.75.75 0 0 1 0 1.5h-2a.75.75 0 0 1 0-1.5h.25v-2h-.25a.75.75 0 0 1-.75-.75ZM8 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"></path>
</svg>"#;

const TIP_ICON: &str = r#"<svg class="markdown-icon markdown-icon-tip mr-2" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  <path d="M8 1.5c-2.363 0-4 1.69-4 3.75 0 .984.424 1.625.984 2.304l.214.253c.223.264.47.556.673.848.284.411.537.896.621 1.49a.75.75 0 0 1-1.484.211c-.04-.282-.163-.547-.37-.847a8.456 8.456 0 0 0-.542-.68c-.084-.1-.173-.205-.268-.32C3.201 7.75 2.5 6.766 2.5 5.25 2.5 2.31 4.863 0 8 0s5.5 2.31 5.5 5.25c0 1.516-.701 2.5-1.328 3.259-.095.115-.184.22-.268.319-.207.245-.383.453-.541.681-.208.3-.33.565-.37.847a.751.751 0 0 1-1.485-.212c.084-.593.337-1.078.621-1.489.203-.292.45-.584.673-.848.075-.088.147-.173.213-.253.561-.679.985-1.32.985-2.304 0-2.06-1.637-3.75-4-3.75ZM5.75 12h4.5a.75.75 0 0 1 0 1.5h-4.5a.75.75 0 0 1 0-1.5ZM6 15.25a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z"></path>
</svg>"#;

const IMPORTANT_ICON: &str = r#"<svg class="markdown-icon markdown-icon-important mr-2" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  <path d="M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5Zm7 2.25v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 9a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"></path>
</svg>"#;

const WARNING_ICON: &str = r#"<svg class="markdown-icon markdown-icon-warning mr-2" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  <path d="M6.457 1.047c.659-1.234 2.427-1.234 3.086 0l6.082 11.378A1.75 1.75 0 0 1 14.082 15H1.918a1.75 1.75 0 0 1-1.543-2.575Zm1.763.707a.25.25 0 0 0-.44 0L1.698 13.132a.25.25 0 0 0 .22.368h12.164a.25.25 0 0 0 .22-.368Zm.53 3.996v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 11a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"></path>
</svg>"#;

const CAUTION_ICON: &str = r#"<svg class="markdown-icon markdown-icon-caution mr-2" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  <path d="M4.47.22A.749.749 0 0 1 5 0h6c.199 0 .389.079.53.22l4.25 4.25c.141.14.22.331.22.53v6a.749.749 0 0 1-.22.53l-4.25 4.25A.749.749 0 0 1 11 16H5a.749.749 0 0 1-.53-.22L.22 11.53A.749.749 0 0 1 0 11V5c0-.199.079-.389.22-.53Zm.84 1.28L1.5 5.31v5.38l3.81 3.81h5.38l3.81-3.81V5.31L10.69 1.5ZM8 4a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5a.75.75 0 0 1 .75-.75Zm0 8a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"></path>
</svg>"#;

/// Icons used for each alert kind by GitHub Flavored Markdown.
pub const GITHUB_ALERT_ICONS: &[(&str, &str)] = &[
    ("NOTE", NOTE_ICON),
    ("TIP", TIP_ICON),
    ("IMPORTANT", IMPORTANT_ICON),
    ("WARNING", WARNING_ICON),
    ("CAUTION", CAUTION_ICON),
];

/// Options for the alert HTML renderer.
#[derive(Debug, Clone, Copy)]
pub struct AlertHtmlRendererOptions {
    /// Icons for each alert kind. Kind matching is case insensitive.
    ///
    /// Icons are rendered as raw HTML, so they should be valid HTML fragments.
    pub icons: &'static [(&'static str, &'static str)],
}

impl Default for AlertHtmlRendererOptions {
    fn default() -> Self {
        Self {
            icons: GITHUB_ALERT_ICONS,
        }
    }
}

impl RendererOptions for AlertHtmlRendererOptions {}

struct AlertHtmlRenderer<W: TextWrite> {
    _phantom: PhantomData<W>,
    options: AlertHtmlRendererOptions,
}

impl<W: TextWrite> AlertHtmlRenderer<W> {
    fn new(options: AlertHtmlRendererOptions) -> Self {
        Self {
            _phantom: PhantomData,
            options,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for AlertHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _context: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if !entering {
            w.write_str("</div>")?;
            return Ok(WalkStatus::Continue);
        }
        let alert = as_extension_data!(arena, node_ref, Alert);
        let kind = alert.kind().str(source);

        w.write_str("<div class=\"markdown-alert markdown-alert-")?;
        html::write_text(w, &kind.to_lowercase())?;
        w.write_str("\">\n<p class=\"markdown-alert-title\">\n")?;
        if let Some((_, icon)) = self
            .options
            .icons
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&kind))
        {
            html::write_html(w, icon)?;
            w.write_str("\n")?;
        }
        if alert.title().is_empty() {
            html::write_text(w, &capitalize(&alert.kind().value(source)))?;
        } else {
            alert.title().write_to(&mut TextWriter::new(w), source)?;
        }
        w.write_str("\n</p>\n")?;
        Ok(WalkStatus::Continue)
    }
}

impl<'cb, W> NodeRenderer<'cb, W> for AlertHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<Alert>(), BoxRenderNode::new(self));
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut result: String = first.to_uppercase().collect();
    result.extend(chars.flat_map(char::to_lowercase));
    result
}

// }}} Renderer

// Extension {{{

/// Returns a parser extension that parses GitHub style alert blocks.
pub fn alert_parser_extension(options: AlertParserOptions) -> impl ParserExtension {
    ParserExtensionFn::new(move |p: &mut Parser| {
        p.add_ast_transformer(AlertAstTransformer::with_options, options, 999);
    })
}

/// Returns a renderer extension that renders alerts as HTML.
pub fn alert_html_renderer_extension<'cb, W>(
    options: AlertHtmlRendererOptions,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    renderer_extension(move |r| {
        r.add_node_renderer(AlertHtmlRenderer::new, options);
    })
}

// }}} Extension
