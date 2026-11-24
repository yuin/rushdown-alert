# rushdown-alert
rushdown-alert is a [GitHub style alert](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#alerts)
extension for [rushdown](https://github.com/yuin/rushdown), a markdown parser.

## Installation
Add dependency to your `Cargo.toml`:

```toml
[dependencies]
rushdown-alert = "x.y.z"
```

rushdown-alert can also be used in `no_std` environments. To enable this feature, add the following line to your `Cargo.toml`:

```toml
rushdown-alert = { version = "x.y.z", default-features = false, features = ["no-std"] }
```

## Usage
### Example

`````rust
use rushdown::{new_markdown_to_html_string, parser, renderer::html};
use rushdown_alert::{
    alert_html_renderer_extension, alert_parser_extension, AlertHtmlRendererOptions,
    AlertParserOptions,
};

let markdown_to_html = new_markdown_to_html_string(
    parser::Options::default(),
    html::Options::default(),
    alert_parser_extension(AlertParserOptions::default()),
    alert_html_renderer_extension(AlertHtmlRendererOptions::default()),
);
let input = "> [!NOTE] Custom title\n>\n> This is a note.\n";
let mut output = String::new();
match markdown_to_html(&mut output, input) {
    Ok(_) => {
        println!("HTML output:\n{}", output);
    }
    Err(e) => {
        println!("Error: {:?}", e);
    }
}
`````

HTML output for the above example:

```html
<div class="markdown-alert markdown-alert-note">
<p class="markdown-alert-title">
<svg ...></svg>
Custom title
</p>
<p>This is a note.</p>
</div>
```

Titles after the kind, like `> [!WARNING] Heads up`, are used as alert titles.

### Options

Parser options (`AlertParserOptions`):

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `allows_nested` | `bool` | `false` | GitHub does not allow nested alerts, but you can enable them. |
| `kinds` | `&'static [&'static str]` | `GITHUB_ALERT_KINDS` | Kinds of alerts to enable. Kind matching is case insensitive. |

HTML renderer options (`AlertHtmlRendererOptions`):

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `icons` | `&'static [(&'static str, &'static str)]` | `GITHUB_ALERT_ICONS` | Icons for each alert kind. Kind matching is case insensitive. Icons are rendered as raw HTML. |

```rust
use rushdown_alert::{
    AlertHtmlRendererOptions, AlertParserOptions,
};

let parser_options = AlertParserOptions {
    allows_nested: true,
    kinds: &["NOTE", "TODO"],
    ..AlertParserOptions::default()
};
let renderer_options = AlertHtmlRendererOptions {
    icons: &[("TODO", "<span>TODO</span>")],
    ..AlertHtmlRendererOptions::default()
};
```

## License
MIT

## Author
Yusuke Inuzuka
