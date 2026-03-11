use rushdown::{
    as_kind_data,
    ast::{Arena, NodeRef},
    new_markdown_to_html_string,
    parser::{
        self, AnyAstTransformer, AstTransformer, NoParserOptions, Parser, ParserExtension,
        ParserExtensionFn,
    },
    renderer::html,
    test::{MarkdownTestCase, MarkdownTestCaseOptions},
    text,
};
use rushdown_meta::{meta_parser_extension, MetaParserOptions};

#[derive(Debug)]
struct TestMeta;

impl TestMeta {
    pub fn new() -> Self {
        Self
    }
}

impl AstTransformer for TestMeta {
    fn transform(
        &self,
        arena: &mut Arena,
        doc_ref: NodeRef,
        _reader: &mut text::BasicReader,
        __ctx: &mut parser::Context,
    ) {
        let meta = as_kind_data!(arena, doc_ref, Document).metadata();
        assert_eq!(
            meta.get("title").unwrap().as_str().unwrap(),
            "YAML Frontmatter"
        );
        assert_eq!(meta.get("date").unwrap().as_str().unwrap(), "2026-03-11");
        assert_eq!(
            meta.get("tags").unwrap().as_sequence().unwrap()[0]
                .as_str()
                .unwrap(),
            "Rust"
        );
        assert_eq!(
            meta.get("tags").unwrap().as_sequence().unwrap()[1]
                .as_str()
                .unwrap(),
            "Markdown<>"
        );
        let author = meta.get("author").unwrap().as_mapping().unwrap();
        assert_eq!(author.get("name").unwrap().as_str().unwrap(), "yuin");
    }
}

impl From<TestMeta> for AnyAstTransformer {
    fn from(t: TestMeta) -> Self {
        AnyAstTransformer::Extension(Box::new(t))
    }
}

#[test]
fn test_meta() {
    let source = r#"---
title: YAML Frontmatter
date: "2026-03-11"
tags: ["Rust", "Markdown<>"]
author:
  name: yuin
---
aaa
"#;
    let markdown_to_html = new_markdown_to_html_string(
        parser::Options::default(),
        html::Options {
            allows_unsafe: true,
            xhtml: false,
            ..html::Options::default()
        },
        meta_parser_extension(MetaParserOptions { table: true }).and(ParserExtensionFn::new(
            |p: &mut Parser| {
                p.add_ast_transformer(TestMeta::new, NoParserOptions, 0);
            },
        )),
        html::NO_EXTENSIONS,
    );
    MarkdownTestCase::new(
        1,
        String::from("ok"),
        String::from(source),
        String::from(
            r#"<table>
<thead>
<tr>
<th>title</th>
<th>date</th>
<th>tags</th>
<th>author</th>
</tr>
</thead>
<tbody>
<tr>
<td>YAML Frontmatter</td>
<td>2026-03-11</td>
<td>[Rust, Markdown&lt;&gt;]</td>
<td>{name: yuin}</td>
</tr>
</tbody>
</table>
<p>aaa</p>
"#,
        ),
        MarkdownTestCaseOptions::default(),
    )
    .execute(&markdown_to_html);
}

#[test]
fn test_error() {
    let source = r#"---
title: YAML Frontmatter
hogehoge
---
aaa
"#;
    let markdown_to_html = new_markdown_to_html_string(
        parser::Options::default(),
        html::Options {
            allows_unsafe: true,
            xhtml: false,
            ..html::Options::default()
        },
        meta_parser_extension(MetaParserOptions::default()),
        html::NO_EXTENSIONS,
    );
    MarkdownTestCase::new(
        1,
        String::from("ok"),
        String::from(source),
        String::from(r#"<!-- Error parsing YAML metadata: YAML parsing error: Terminate { name: "map splitter", msg: "2:9\nhogehoge\n        ^" } -->
<p>aaa</p>
"#),
        MarkdownTestCaseOptions::default(),
    )
    .execute(&markdown_to_html);
}

#[test]
fn test_ok() {
    let source = r#"---
title: YAML Frontmatter
---
aaa
"#;
    let parser = Parser::with_extensions(
        parser::Options::default(),
        meta_parser_extension(MetaParserOptions::default()),
    );
    let renderer = html::Renderer::with_extensions(html::Options::default(), html::NO_EXTENSIONS);
    let mut reader = text::BasicReader::new(source);
    let (arena, document_ref) = parser.parse(&mut reader);
    let metadata = as_kind_data!(&arena, document_ref, Document).metadata();
    assert_eq!(
        metadata.get("title").unwrap().as_str().unwrap(),
        "YAML Frontmatter"
    );
    let mut output = String::new();
    renderer
        .render(&mut output, source, &arena, document_ref)
        .expect("Rendering failed");
}
