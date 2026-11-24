use rushdown::{new_markdown_to_html_string, parser, renderer::html};
use rushdown_alert::{
    alert_html_renderer_extension, alert_parser_extension, AlertHtmlRendererOptions,
    AlertParserOptions, GITHUB_ALERT_ICONS,
};

fn icon(kind: &str) -> &'static str {
    GITHUB_ALERT_ICONS
        .iter()
        .find(|(k, _)| *k == kind)
        .expect("unknown kind")
        .1
}

fn render(
    source: &str,
    parser_options: AlertParserOptions,
    renderer_options: AlertHtmlRendererOptions,
) -> String {
    let markdown_to_html = new_markdown_to_html_string(
        parser::Options::default(),
        html::Options::default(),
        alert_parser_extension(parser_options),
        alert_html_renderer_extension(renderer_options),
    );
    let mut output = String::new();
    markdown_to_html(&mut output, source).expect("rendering failed");
    output.trim().to_string()
}

fn assert_rendered(
    source: &str,
    parser_options: AlertParserOptions,
    renderer_options: AlertHtmlRendererOptions,
    expected: &str,
) {
    let actual = render(source, parser_options, renderer_options);
    assert_eq!(expected.trim(), actual);
}

#[test]
fn test_alert() {
    let source = r#"## Title

> [!TIP]
>
> > [!NOTE]
> > This is a note inside a tip.
>
> hoge
"#;
    let expected = format!(
        r#"<h2>Title</h2>
<div class="markdown-alert markdown-alert-tip">
<p class="markdown-alert-title">
{}
Tip
</p>
<div class="markdown-alert markdown-alert-note">
<p class="markdown-alert-title">
{}
Note
</p>
<p>This is a note inside a tip.</p>
</div><p>hoge</p>
</div>"#,
        icon("TIP"),
        icon("NOTE")
    );
    assert_rendered(
        source,
        AlertParserOptions {
            allows_nested: true,
            ..AlertParserOptions::default()
        },
        AlertHtmlRendererOptions::default(),
        &expected,
    );
}

#[test]
fn test_alert_default_title() {
    let source = r#"> [!note]
> body
"#;
    let expected = format!(
        r#"<div class="markdown-alert markdown-alert-note">
<p class="markdown-alert-title">
{}
Note
</p>
<p>body</p>
</div>"#,
        icon("NOTE")
    );
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions::default(),
        &expected,
    );
}

#[test]
fn test_alert_custom_title() {
    let source = r#"> [!WARNING] Heads up
> body
"#;
    let expected = format!(
        r#"<div class="markdown-alert markdown-alert-warning">
<p class="markdown-alert-title">
{}
Heads up
</p>
<p>body</p>
</div>"#,
        icon("WARNING")
    );
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions::default(),
        &expected,
    );
}

#[test]
fn test_alert_nested_disabled_by_default() {
    let source = r#"> [!TIP]
>
> > [!NOTE]
> > body
"#;
    let expected = format!(
        r#"<div class="markdown-alert markdown-alert-tip">
<p class="markdown-alert-title">
{}
Tip
</p>
<blockquote>
<p>[!NOTE]
body</p>
</blockquote>
</div>"#,
        icon("TIP")
    );
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions::default(),
        &expected,
    );
}

#[test]
fn test_alert_custom_kinds() {
    let source = r#"> [!todo] Work
> body
"#;
    let expected = r#"<div class="markdown-alert markdown-alert-todo">
<p class="markdown-alert-title">
Work
</p>
<p>body</p>
</div>"#;
    assert_rendered(
        source,
        AlertParserOptions {
            kinds: &["TODO"],
            ..AlertParserOptions::default()
        },
        AlertHtmlRendererOptions::default(),
        expected,
    );
}

#[test]
fn test_alert_custom_icons() {
    let source = r#"> [!NOTE]
> body
"#;
    let expected = r#"<div class="markdown-alert markdown-alert-note">
<p class="markdown-alert-title">
<span>ICON</span>
Note
</p>
<p>body</p>
</div>"#;
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions {
            icons: &[("NOTE", "<span>ICON</span>")],
        },
        expected,
    );
}

#[test]
fn test_alert_invalid_kinds_remain_blockquotes() {
    let source = r#"> [!todo]
> body
"#;
    let expected = r#"<blockquote>
<p>[!todo]
body</p>
</blockquote>"#;
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions::default(),
        expected,
    );
}

#[test]
fn test_alert_title_with_escape() {
    let source = "> [!NOTE] a \\*b\\* c\n> body\n";
    let expected = format!(
        r#"<div class="markdown-alert markdown-alert-note">
<p class="markdown-alert-title">
{}
a *b* c
</p>
<p>body</p>
</div>"#,
        icon("NOTE")
    );
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions::default(),
        &expected,
    );
}

#[test]
fn test_alert_title_split_by_inline() {
    let source = "> [!NOTE] a [x] c\n> body\n";
    let expected = format!(
        r#"<div class="markdown-alert markdown-alert-note">
<p class="markdown-alert-title">
{}
a [x] c
</p>
<p>body</p>
</div>"#,
        icon("NOTE")
    );
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions::default(),
        &expected,
    );
}

#[test]
fn test_alert_title_with_inline_node() {
    let source = "> [!NOTE] a *b* c\n> body\n";
    let expected = format!(
        r#"<div class="markdown-alert markdown-alert-note">
<p class="markdown-alert-title">
{}
a  c
</p>
<p><em>b</em>body</p>
</div>"#,
        icon("NOTE")
    );
    assert_rendered(
        source,
        AlertParserOptions::default(),
        AlertHtmlRendererOptions::default(),
        &expected,
    );
}
