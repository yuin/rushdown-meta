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
    parser::{self, Parser, ParserExtension },
    renderer::html,
    text,
};
use rushdown_meta::{meta_parser_extension, MetaParserOptions};

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
```

Metadata is stored in the Document node.
You can get the metadata from the Document using `metadata()` method.

### Options

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `table`| `bool` | `false` | whether to render the meta as a table |

## Donation
BTC: 1NEDSyUmo4SMTDP83JJQSWi1MvQUGGNMZB

Github sponsors also welcome.

## License
MIT

## Author
Yusuke Inuzuka
