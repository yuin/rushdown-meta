#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt;
use core::fmt::Write;
use core::marker::PhantomData;
use core::result::Result as CoreResult;

use rushdown::{
    as_extension_data, as_extension_data_mut, as_kind_data_mut,
    ast::{
        pp_indent, Arena, InlineChars, KindData, Meta, NodeKind, NodeRef, NodeType, PrettyPrint,
        WalkStatus,
    },
    parser::{
        self, AnyBlockParser, BlockParser, NoParserOptions, Parser, ParserExtension,
        ParserExtensionFn,
    },
    renderer::{
        self,
        html::{self, renderer_extension, RendererExtension},
        BoxRenderNode, NodeRenderer, NodeRendererRegistry, RenderNode, RendererOptions, TextWrite,
    },
    text::{self, Reader},
    util::{self, is_blank, StringMap},
    Result,
};

// AST {{{

/// A struct representing a meta(YAML frontmatter) block in the AST.
#[derive(Debug)]
pub struct MetaBlock {
    value: Vec<text::Segment>,
    meta: Meta,
    error: Option<String>,
}

impl MetaBlock {
    fn new() -> Self {
        Self {
            value: Vec::new(),
            meta: Meta::Null,
            error: None,
        }
    }

    /// Returns the raw source lines of this block.
    #[inline(always)]
    pub fn value(&self) -> &[text::Segment] {
        &self.value
    }

    /// Returns the metadata parsed from this block.
    ///
    /// Returns `Meta::Null` if the block could not be parsed.
    #[inline(always)]
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Returns the error message if this block could not be parsed.
    #[inline(always)]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl InlineChars for MetaBlock {}

impl NodeKind for MetaBlock {
    fn typ(&self) -> NodeType {
        NodeType::LeafBlock
    }

    fn kind_name(&self) -> &'static str {
        "MetaBlock"
    }
}

impl PrettyPrint for MetaBlock {
    fn pretty_print(&self, w: &mut dyn Write, source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}Value:", pp_indent(level))?;
        for segment in self.value() {
            writeln!(w, "{}  {}", pp_indent(level), segment.str(source))?;
        }
        writeln!(w, "{}Meta: {:?}", pp_indent(level), self.meta())
    }
}

impl From<MetaBlock> for KindData {
    fn from(e: MetaBlock) -> Self {
        KindData::Extension(Box::new(e))
    }
}

// }}}

// Parser {{{

/// Returns true if the given line consists of only `-` and spaces.
fn is_separator(line: &[u8]) -> bool {
    util::trim_right_space(util::trim_left_space(line))
        .iter()
        .all(|b| *b == b'-')
}

#[derive(Debug, Default)]
struct MetaParser {}

impl MetaParser {
    fn new() -> Self {
        Self {}
    }
}

impl BlockParser for MetaParser {
    fn trigger(&self) -> &[u8] {
        b"-"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        let (line, _) = reader.position();
        if line != 0 {
            return None;
        }
        let (line, _) = reader.peek_line_bytes()?;
        if !is_separator(&line) {
            return None;
        }
        reader.advance_to_eol();
        Some((arena.new_node(MetaBlock::new()), parser::State::NO_CHILDREN))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut parser::Context,
    ) -> Option<parser::State> {
        let (line, segment) = reader.peek_line_bytes()?;
        if !is_blank(&line) && is_separator(&line) {
            reader.advance_to_eol();
            return None;
        }
        as_extension_data_mut!(arena, node_ref, MetaBlock)
            .value
            .push(segment);
        Some(parser::State::NO_CHILDREN)
    }

    fn close(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut parser::Context,
    ) {
        let source = reader.source();
        let yaml = {
            let mut buf = String::new();
            for segment in as_extension_data!(arena, node_ref, MetaBlock).value() {
                buf.push_str(&segment.str(source));
            }
            buf
        };
        let (meta, error) = match parse_yaml(&yaml) {
            Ok(meta) => (meta, None),
            Err(e) => (Meta::Null, Some(e)),
        };
        if let Some(doc_ref) = arena[node_ref].owner_document(arena) {
            if let Meta::Mapping(map) = &meta {
                let metadata = as_kind_data_mut!(arena, doc_ref, Document).metadata_mut();
                for (key, value) in map.iter() {
                    metadata.insert(key.clone(), value.clone());
                }
            }
        }
        let data = as_extension_data_mut!(arena, node_ref, MetaBlock);
        data.meta = meta;
        data.error = error;
    }

    fn can_interrupt_paragraph(&self) -> bool {
        false
    }
}

impl From<MetaParser> for AnyBlockParser {
    fn from(p: MetaParser) -> Self {
        AnyBlockParser::Extension(Box::new(p))
    }
}

// }}}

// Renderer {{{

/// A layout of the meta table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetaTableLayout {
    /// Renders keys as table columns.
    #[default]
    Columns,
    /// Renders keys as table rows.
    Rows,
}

/// Options for the meta HTML renderer.
#[derive(Debug, Clone, Default)]
pub struct MetaHtmlRendererOptions {
    /// Renders parsed metadata as an HTML table before the document body.
    pub table: Option<MetaTableLayout>,
}

impl MetaHtmlRendererOptions {
    /// Returns options that render the meta table with the given layout.
    pub fn with_table(layout: MetaTableLayout) -> Self {
        Self {
            table: Some(layout),
        }
    }
}

impl RendererOptions for MetaHtmlRendererOptions {}

struct MetaBlockHtmlRenderer<W: TextWrite> {
    _phantom: PhantomData<W>,
    options: MetaHtmlRendererOptions,
}

impl<W: TextWrite> MetaBlockHtmlRenderer<W> {
    fn new(options: MetaHtmlRendererOptions) -> Self {
        Self {
            _phantom: PhantomData,
            options,
        }
    }

    fn render_table(w: &mut W, map: &StringMap<Meta>, layout: MetaTableLayout) -> Result<()> {
        match layout {
            MetaTableLayout::Columns => {
                w.write_str("<table>\n<thead>\n<tr>\n")?;
                for (key, _) in map.iter() {
                    w.write_str("<th>")?;
                    html::write_text(w, key)?;
                    w.write_str("</th>\n")?;
                }
                w.write_str("</tr>\n</thead>\n<tbody>\n<tr>\n")?;
                for (_, value) in map.iter() {
                    w.write_str("<td>")?;
                    render_meta_value(w, value)?;
                    w.write_str("</td>\n")?;
                }
                w.write_str("</tr>\n</tbody>\n</table>\n")
            }
            MetaTableLayout::Rows => {
                w.write_str("<table>\n<tbody>\n")?;
                for (key, value) in map.iter() {
                    w.write_str("<tr>\n<td>")?;
                    html::write_text(w, key)?;
                    w.write_str("</td>\n<td>")?;
                    render_meta_value(w, value)?;
                    w.write_str("</td>\n</tr>\n")?;
                }
                w.write_str("</tbody>\n</table>\n")
            }
        }
    }
}

impl<W: TextWrite> RenderNode<W> for MetaBlockHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        _source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _context: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if !entering {
            return Ok(WalkStatus::SkipChildren);
        }
        let data = as_extension_data!(arena, node_ref, MetaBlock);
        if let Some(error) = data.error() {
            w.write_str("<!-- ")?;
            w.write_str(error)?;
            w.write_str(" -->\n")?;
            return Ok(WalkStatus::SkipChildren);
        }
        let Some(layout) = self.options.table else {
            return Ok(WalkStatus::SkipChildren);
        };
        let Meta::Mapping(map) = data.meta() else {
            return Ok(WalkStatus::SkipChildren);
        };
        Self::render_table(w, map, layout)?;
        Ok(WalkStatus::SkipChildren)
    }
}

impl<'cb, W> NodeRenderer<'cb, W> for MetaBlockHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<MetaBlock>(), BoxRenderNode::new(self));
    }
}

fn format_scalar(meta: &Meta) -> String {
    match meta {
        Meta::Bool(b) => b.to_string(),
        Meta::Int(i) => i.to_string(),
        Meta::Float(f) => f.to_string(),
        Meta::String(s) => s.clone(),
        _ => "null".to_string(),
    }
}

fn render_meta_value<W: TextWrite + ?Sized>(w: &mut W, meta: &Meta) -> Result<()> {
    match meta {
        Meta::Sequence(items) => {
            w.write_str("<ul>")?;
            for item in items {
                w.write_str("<li>")?;
                render_meta_value(w, item)?;
                w.write_str("</li>")?;
            }
            w.write_str("</ul>")
        }
        Meta::Mapping(map) => {
            w.write_str("<ul>")?;
            for (key, value) in map.iter() {
                w.write_str("<li>")?;
                html::write_text(w, key)?;
                w.write_str(": ")?;
                render_meta_value(w, value)?;
                w.write_str("</li>")?;
            }
            w.write_str("</ul>")
        }
        _ => html::write_text(w, &format_scalar(meta)),
    }
}

// }}}

// Extension {{{

/// Returns a parser extension that parses meta(YAML frontmatter) blocks.
pub fn meta_parser_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_block_parser(MetaParser::new, NoParserOptions, 0);
    })
}

/// Returns a renderer extension that renders meta blocks as HTML.
pub fn meta_html_renderer_extension<'cb, W>(
    options: impl Into<MetaHtmlRendererOptions>,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    let options = options.into();
    renderer_extension(move |r| {
        r.add_node_renderer(MetaBlockHtmlRenderer::new, options);
    })
}

// }}}

// YAML {{{

fn to_meta<R: yaml_peg::repr::Repr>(node: &yaml_peg::Node<R>) -> Meta {
    match node.yaml() {
        yaml_peg::Yaml::Null => Meta::Null,
        yaml_peg::Yaml::Bool(b) => Meta::Bool(*b),
        yaml_peg::Yaml::Int(s) => Meta::Int(s.parse().unwrap_or(0)),
        yaml_peg::Yaml::Float(s) => Meta::Float(s.parse().unwrap_or(0.0)),
        yaml_peg::Yaml::Str(s) => Meta::String(s.clone()),
        yaml_peg::Yaml::Seq(seq) => Meta::Sequence(seq.iter().map(to_meta).collect()),
        yaml_peg::Yaml::Map(map) => {
            let mut result = StringMap::with_capacity(map.len());
            for (k, v) in map.iter() {
                if let yaml_peg::Yaml::Str(key) = k.yaml() {
                    result.insert(key.clone(), to_meta(v));
                }
            }
            Meta::Mapping(result)
        }
        yaml_peg::Yaml::Alias(_) => Meta::Null,
    }
}

fn parse_yaml(input: &str) -> CoreResult<Meta, String> {
    let doc = yaml_peg::parser::parse::<yaml_peg::repr::RcRepr>(input)
        .map_err(|e| format!("YAML parsing error: {:?}", e))?;
    let Some(node) = doc.first() else {
        return Err("YAML document is empty".to_string());
    };
    let meta = to_meta(node);
    if matches!(meta, Meta::Mapping(_)) {
        Ok(meta)
    } else {
        Err("YAML metadata must be a mapping".to_string())
    }
}

// }}} YAML
