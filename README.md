# rushdown-meta
rushdown-meta is a simple meta(YAML frontmatter) plugin for [rushdown](https://github.com/yuin/rushdown), a markdown parser. It allows you to easily add metas to your markdown documents.

## Installation
Add dependency to your `Cargo.toml`:

```toml
[dependencies]
rushdown-meta = "x.y.z"
```

rushdown-meta can also be used in `no_std` environments. To enable this feature, add the following line to your `Cargo.toml`:

```toml
rushdown-meta = { version = "x.y.z", default-features = false, features = ["no-std"] }
```

## Syntax

```markdown
---
date: 2024-01-01
tags:
    - rust
    - markdown
title: My Document
---

That's some text with a meta.
```

## Usage
### Example

```rust
use rushdown::{
    as_kind_data,
    ast::Document,
    parser::{self, ParseOptions, Parser},
    renderer::{html, RenderOptions},
};
use rushdown_meta::{meta_html_renderer_extension, meta_parser_extension, MetaHtmlRendererOptions};

let source = r#"---
title: YAML Frontmatter
---
aaa
"#;
let parser = Parser::with_extensions(parser::Options::default(), meta_parser_extension());
let (arena, document_ref) = parser.parse(source, ParseOptions::default());
let metadata = as_kind_data!(&arena, document_ref, Document).metadata();
assert_eq!(
    metadata.get("title").unwrap().as_str().unwrap(),
    "YAML Frontmatter"
);
let renderer = html::Renderer::with_extensions(
    html::Options::default(),
    meta_html_renderer_extension(MetaHtmlRendererOptions::default()),
);
let mut output = String::new();
renderer
    .render(&mut output, source, &arena, document_ref, RenderOptions::default())
    .expect("Rendering Failed");
assert_eq!(output, "<p>aaa</p>\n");
```

Metadata is stored in the Document node.
You can get the metadata from the Document using `metadata()` method.

The `MetaBlock` node is kept in the AST, and it renders nothing by default.
If YAML parsing fails, the `MetaBlock` renders an HTML comment that describes the error, and no metadata is stored in the Document.

### Options

`MetaHtmlRendererOptions` is an option of `meta_html_renderer_extension`.

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `table` | `Option<MetaTableLayout>` | `None` | whether to render the meta as a table |

`MetaTableLayout` is either `Columns` or `Rows`.

## Donation
BTC: 1NEDSyUmo4SMTDP83JJQSWi1MvQUGGNMZB

Github sponsors also welcome.

## License
MIT

## Author
Yusuke Inuzuka
