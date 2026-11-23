use rushdown::{
    as_kind_data,
    ast::{Arena, KindData, NodeRef},
    new_markdown_to_html_string,
    parser::{self, ParseOptions, Parser},
    renderer::html,
};
use rushdown_meta::{
    meta_html_renderer_extension, meta_parser_extension, MetaHtmlRendererOptions, MetaTableLayout,
};

const SOURCE: &str = r#"---
Title: goldmark-meta
Summary: Add YAML metadata to the document
Tags:
    - markdown
    - goldmark
---

# Hello goldmark-meta
"#;

fn parse(source: &str) -> (Arena, NodeRef) {
    let parser = Parser::with_extensions(parser::Options::default(), meta_parser_extension());
    parser.parse(source, ParseOptions::default())
}

fn render(source: &str, options: MetaHtmlRendererOptions) -> String {
    let markdown_to_html = new_markdown_to_html_string(
        parser::Options::default(),
        html::Options {
            allows_unsafe: true,
            xhtml: false,
            ..html::Options::default()
        },
        meta_parser_extension(),
        meta_html_renderer_extension(options),
    );
    let mut output = String::new();
    markdown_to_html(&mut output, source).expect("rendering failed");
    output
}

#[test]
fn test_meta() {
    let (arena, doc_ref) = parse(SOURCE);
    let meta_block_ref = arena[doc_ref].first_child().expect("meta block");
    assert!(matches!(
        arena[meta_block_ref].kind_data(),
        KindData::Extension(_)
    ));
    assert_eq!(
        as_kind_data!(arena, doc_ref, Document)
            .metadata()
            .get("Title")
            .unwrap()
            .as_str()
            .unwrap(),
        "goldmark-meta"
    );
    let tags = as_kind_data!(arena, doc_ref, Document)
        .metadata()
        .get("Tags")
        .unwrap()
        .as_sequence()
        .unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].as_str().unwrap(), "markdown");
    assert_eq!(tags[1].as_str().unwrap(), "goldmark");
    assert_eq!(
        render(SOURCE, MetaHtmlRendererOptions::default()),
        "<h1>Hello goldmark-meta</h1>\n"
    );
}

#[test]
fn test_meta_table() {
    let expected = r#"<table>
<thead>
<tr>
<th>Title</th>
<th>Summary</th>
<th>Tags</th>
</tr>
</thead>
<tbody>
<tr>
<td>goldmark-meta</td>
<td>Add YAML metadata to the document</td>
<td><ul><li>markdown</li><li>goldmark</li></ul></td>
</tr>
</tbody>
</table>
<h1>Hello goldmark-meta</h1>
"#;
    assert_eq!(
        render(
            SOURCE,
            MetaHtmlRendererOptions::with_table(MetaTableLayout::Columns)
        ),
        expected
    );
}

#[test]
fn test_meta_table_rows() {
    let expected = r#"<table>
<tbody>
<tr>
<td>Title</td>
<td>goldmark-meta</td>
</tr>
<tr>
<td>Summary</td>
<td>Add YAML metadata to the document</td>
</tr>
<tr>
<td>Tags</td>
<td><ul><li>markdown</li><li>goldmark</li></ul></td>
</tr>
</tbody>
</table>
<h1>Hello goldmark-meta</h1>
"#;
    assert_eq!(
        render(
            SOURCE,
            MetaHtmlRendererOptions::with_table(MetaTableLayout::Rows)
        ),
        expected
    );
}

#[test]
fn test_meta_table_with_blank_line() {
    let source = r#"---
Title: goldmark-meta
Summary: Add YAML metadata to the document

# comments
Tags:
    - markdown
    - goldmark
---

# Hello goldmark-meta
"#;
    let expected = r#"<table>
<thead>
<tr>
<th>Title</th>
<th>Summary</th>
<th>Tags</th>
</tr>
</thead>
<tbody>
<tr>
<td>goldmark-meta</td>
<td>Add YAML metadata to the document</td>
<td><ul><li>markdown</li><li>goldmark</li></ul></td>
</tr>
</tbody>
</table>
<h1>Hello goldmark-meta</h1>
"#;
    assert_eq!(
        render(
            source,
            MetaHtmlRendererOptions::with_table(MetaTableLayout::Columns)
        ),
        expected
    );
}

#[test]
fn test_meta_error() {
    let source = r#"---
Title: goldmark-meta
Summary: Add YAML metadata to the document
Tags:
  - : {
  }
    - markdown
---

# Hello goldmark-meta
"#;
    let (arena, doc_ref) = parse(source);
    assert!(as_kind_data!(arena, doc_ref, Document)
        .metadata()
        .get("Title")
        .is_none());
    let output = render(
        source,
        MetaHtmlRendererOptions::with_table(MetaTableLayout::Columns),
    );
    assert!(output.starts_with("<!-- YAML parsing error: "), "{output}");
    assert!(
        output.ends_with(" -->\n<h1>Hello goldmark-meta</h1>\n"),
        "{output}"
    );
}

#[test]
fn test_meta_is_not_a_mapping() {
    let source = r#"---
- markdown
- goldmark
---
aaa
"#;
    let (arena, doc_ref) = parse(source);
    assert_eq!(as_kind_data!(arena, doc_ref, Document).metadata().len(), 0);
    assert_eq!(
        render(source, MetaHtmlRendererOptions::default()),
        "<!-- YAML metadata must be a mapping -->\n<p>aaa</p>\n"
    );
}

#[test]
fn test_meta_only_at_first_line() {
    let source = r#"aaa
---
title: rushdown-meta
---
"#;
    let (arena, doc_ref) = parse(source);
    assert_eq!(as_kind_data!(arena, doc_ref, Document).metadata().len(), 0);
    assert_eq!(
        render(source, MetaHtmlRendererOptions::default()),
        "<h2>aaa</h2>\n<h2>title: rushdown-meta</h2>\n"
    );
}
