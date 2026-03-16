#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::result::Result as CoreResult;
use rushdown::ast::Table;
use rushdown::ast::TableBody;
use rushdown::ast::TableCell;
use rushdown::ast::TableHeader;
use rushdown::ast::TableRow;
use rushdown::parser::ParserOptions;

use rushdown::{
    as_kind_data_mut, as_type_data, as_type_data_mut,
    ast::{Arena, CodeBlock, CodeBlockType, Meta, NodeRef, Text, TextQualifier},
    context::{ContextKey, ContextKeyRegistry, NodeRefValue},
    parser::{
        self, AnyAstTransformer, AnyBlockParser, AstTransformer, BlockParser, NoParserOptions,
        Parser, ParserExtension, ParserExtensionFn, PRIORITY_SETTEXT_HEADING,
    },
    text::{self, Reader},
    util::StringMap,
};

// Parser {{{

const META_NODE: &str = "rushdown-meta-n";

/// Options for the meta parser.
#[derive(Debug, Clone, Default)]
pub struct MetaParserOptions {
    /// Convert the meta data to a table node.
    pub table: bool,
}

impl ParserOptions for MetaParserOptions {}

#[derive(Debug)]
struct MetaParser {
    meta_node: ContextKey<NodeRefValue>,
}

impl MetaParser {
    /// Returns a new [`MetaParser`].
    pub fn new(reg: Rc<RefCell<ContextKeyRegistry>>) -> Self {
        let meta_node = reg.borrow_mut().get_or_create::<NodeRefValue>(META_NODE);
        Self { meta_node }
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
        ctx: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        let (line, _) = reader.position();
        if line != 0 {
            return None;
        }
        let (line, _) = reader.peek_line_bytes()?;
        if !line.starts_with(b"---") {
            return None;
        }
        reader.advance_to_eol();
        let node_ref = arena.new_node(CodeBlock::new(CodeBlockType::Fenced, None));
        ctx.insert(self.meta_node, node_ref);
        Some((node_ref, parser::State::NO_CHILDREN))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut parser::Context,
    ) -> Option<parser::State> {
        let (line, seg) = reader.peek_line_bytes()?;
        if line.starts_with(b"---") {
            reader.advance_to_eol();
            return None;
        }
        as_type_data_mut!(arena, node_ref, Block).append_source_line(seg);
        Some(parser::State::NO_CHILDREN)
    }

    fn close(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        _reader: &mut text::BasicReader,
        _ctx: &mut parser::Context,
    ) {
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

impl From<MetaParser> for AnyBlockParser {
    fn from(p: MetaParser) -> Self {
        AnyBlockParser::Extension(Box::new(p))
    }
}

#[derive(Debug)]
struct MetaAstTransformer {
    meta_node: ContextKey<NodeRefValue>,
    options: MetaParserOptions,
}

impl MetaAstTransformer {
    /// Returns a new [`MetaAstTransformer`].
    pub fn new(reg: Rc<RefCell<ContextKeyRegistry>>, options: MetaParserOptions) -> Self {
        let meta_node = reg.borrow_mut().get_or_create::<NodeRefValue>(META_NODE);
        Self { meta_node, options }
    }
}

impl AstTransformer for MetaAstTransformer {
    fn transform(
        &self,
        arena: &mut Arena,
        doc_ref: NodeRef,
        reader: &mut text::BasicReader,
        ctx: &mut parser::Context,
    ) {
        let Some(meta_ref) = ctx.get(self.meta_node) else {
            return;
        };
        let mut yaml_data = String::new();

        for line in as_type_data!(arena, *meta_ref, Block).source() {
            yaml_data.push_str(&line.str(reader.source()));
        }
        meta_ref.delete(arena);
        match parse_yaml(&yaml_data) {
            Ok(meta) => {
                if let Meta::Mapping(map) = meta {
                    let m = map.clone();
                    for (key, value) in map {
                        as_kind_data_mut!(arena, doc_ref, Document)
                            .metadata_mut()
                            .insert(key, value);
                    }
                    if self.options.table {
                        let table_ref = arena.new_node(Table::new());
                        let header_ref = arena.new_node(TableHeader::new());
                        let header_row_ref = arena.new_node(TableRow::new());
                        for (key, _) in m.iter() {
                            let cell_ref = arena.new_node(TableCell::default());
                            let text_ref = arena
                                .new_node(Text::with_qualifiers(key.clone(), TextQualifier::CODE));
                            cell_ref.append_child(arena, text_ref);
                            header_row_ref.append_child(arena, cell_ref);
                        }
                        header_ref.append_child(arena, header_row_ref);
                        table_ref.append_child(arena, header_ref);

                        let body_ref = arena.new_node(TableBody::new());
                        table_ref.append_child(arena, body_ref);
                        let body_row_ref = arena.new_node(TableRow::new());
                        for (_, value) in m {
                            let cell_ref = arena.new_node(TableCell::default());
                            let text_ref = arena.new_node(Text::new(format_meta(&value)));
                            cell_ref.append_child(arena, text_ref);
                            body_row_ref.append_child(arena, cell_ref);
                        }
                        body_ref.append_child(arena, body_row_ref);
                        if let Some(first) = arena[doc_ref].first_child() {
                            doc_ref.insert_before(arena, first, table_ref);
                        } else {
                            doc_ref.append_child(arena, table_ref);
                        }
                    }
                } else {
                    let error_msg = "<!-- YAML metadata must be a mapping -->\n".to_string();
                    let error_ref =
                        arena.new_node(Text::with_qualifiers(error_msg, TextQualifier::CODE));
                    if let Some(first) = arena[doc_ref].first_child() {
                        doc_ref.insert_before(arena, first, error_ref);
                    } else {
                        doc_ref.append_child(arena, error_ref);
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("<!-- Error parsing YAML metadata: {} -->\n", e);
                let error_ref =
                    arena.new_node(Text::with_qualifiers(error_msg, TextQualifier::CODE));
                if let Some(first) = arena[doc_ref].first_child() {
                    doc_ref.insert_before(arena, first, error_ref);
                } else {
                    doc_ref.append_child(arena, error_ref);
                }
            }
        }
    }
}

impl From<MetaAstTransformer> for AnyAstTransformer {
    fn from(t: MetaAstTransformer) -> Self {
        AnyAstTransformer::Extension(Box::new(t))
    }
}

fn format_meta(meta: &Meta) -> String {
    match meta {
        Meta::Null => "null".to_string(),
        Meta::Bool(b) => b.to_string(),
        Meta::Int(i) => i.to_string(),
        Meta::Float(f) => f.to_string(),
        Meta::String(s) => s.clone(),
        Meta::Sequence(seq) => {
            let items: Vec<String> = seq.iter().map(format_meta).collect();
            format!("[{}]", items.join(", "))
        }
        Meta::Mapping(map) => {
            let items: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_meta(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

// }}}

// Extension {{{

/// Returns a parser extension that parses metas.
pub fn meta_parser_extension(options: MetaParserOptions) -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_block_parser(
            MetaParser::new,
            NoParserOptions,
            PRIORITY_SETTEXT_HEADING - 100,
        );
        p.add_ast_transformer(MetaAstTransformer::new, options, 0);
    })
}

/*
/// Returns a renderer extension that renders metas in HTML.
pub fn meta_html_renderer_extension<'cb, W>(
    options: impl Into<FootnoteHtmlRendererOptions>,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut Renderer<'cb, W>| {
        let options = options.into();
        r.add_post_render_hook(FootnotePostRenderHook::new, options.clone(), 500);
        r.add_node_renderer(FootnoteDefinitionHtmlRenderer::new, options.clone());
        r.add_node_renderer(FootnoteReferenceHtmlRenderer::new, options);
    })
}
*/

// }}}

// YAML {{{

fn to_meta<R: yaml_peg::repr::Repr>(node: &yaml_peg::Node<R>) -> Meta {
    match node.yaml() {
        yaml_peg::Yaml::Null => Meta::Null,
        yaml_peg::Yaml::Bool(b) => Meta::Bool(*b),
        yaml_peg::Yaml::Int(s) => Meta::Int(s.parse().unwrap_or(0)),
        yaml_peg::Yaml::Float(s) => Meta::Float(s.parse().unwrap_or(0.0)),
        yaml_peg::Yaml::Str(s) => Meta::String(s.clone()),
        yaml_peg::Yaml::Seq(seq) => Meta::Sequence(seq.iter().map(|n| to_meta(n)).collect()),
        yaml_peg::Yaml::Map(map) => {
            let mut result = StringMap::with_capacity(map.len());
            for (k, v) in map.iter() {
                if let yaml_peg::Yaml::Str(key) = k.yaml() {
                    result.insert(key.clone(), to_meta(v));
                }
            }
            Meta::Mapping(result)
        }
        yaml_peg::Yaml::Alias(_) => Meta::Null, // Aliases are not supported in this
                                                // implementation
    }
}

fn parse_yaml(input: &str) -> CoreResult<Meta, String> {
    let doc = yaml_peg::parser::parse::<yaml_peg::repr::RcRepr>(input)
        .map_err(|e| format!("YAML parsing error: {:?}", e))?;
    if !doc.is_empty() {
        Ok(to_meta(&doc[0]))
    } else {
        Err("YAML document is empty".to_string())
    }
}

// }}} YAML
