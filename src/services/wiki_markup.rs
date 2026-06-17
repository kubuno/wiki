//! Wiki markup rendering pipeline (MediaWiki-inspired, Markdown-based).
//!
//! Source → HTML in ordered passes:
//!   1. protect code / `<nowiki>` spans
//!   2. extract `#REDIRECT`, magic words
//!   3. expand `{{templates}}` (transclusion, params, parser functions)
//!   4. extract `[[Category:…]]`, resolve `[[internal links]]` (red links)
//!   5. inline references `<ref>…</ref>`
//!   6. Markdown → HTML (pulldown-cmark)
//!   7. heading anchors + table of contents
//!   8. sanitize (ammonia)

use regex::Regex;
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::WikiError;
use crate::models::page::{self, canonical_namespace, slugify, split_namespace};
use crate::services::content_files;
use crate::state::AppState;

/// A resolved internal link reference (for the page_links graph).
#[derive(Debug, Clone)]
pub struct LinkRef {
    pub namespace: String,
    pub title:     String,
    pub slug:      String,
}

/// A category the page belongs to.
#[derive(Debug, Clone)]
pub struct CategoryRef {
    pub title: String,
    pub slug:  String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TocEntry {
    pub level: u8,
    pub text:  String,
    pub id:    String,
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub html:       String,
    pub categories: Vec<CategoryRef>,
    pub links:      Vec<LinkRef>,
    pub redirect:   Option<String>,
    pub toc:        Vec<TocEntry>,
}

/// Render context: knows which wiki/page we render so links resolve correctly.
pub struct Ctx<'a> {
    pub state:     &'a AppState,
    pub wiki_id:   Uuid,
    pub namespace: String,
    pub title:     String,
    pub max_depth: u32,
}

const ITER_CAP: usize = 600;

// ── Public entry point ──────────────────────────────────────────────────────

pub async fn render(ctx: &Ctx<'_>, source: &str) -> Result<RenderResult, WikiError> {
    // 1. Protect code blocks / nowiki from wiki processing.
    let mut protected: Vec<String> = Vec::new();
    let text = protect_code(source, &mut protected);

    // 2. Redirect + magic words.
    let (text, redirect) = extract_redirect(&text);
    let text = substitute_magic_words(ctx, &text);

    // 3. Template expansion (transclusion + parser functions).
    let text = expand_templates(ctx, text, 0).await?;

    // 4. Categories + internal links.
    let mut categories: Vec<CategoryRef> = Vec::new();
    let mut links: Vec<LinkRef> = Vec::new();
    let text = extract_categories(&text, &mut categories);
    let text = resolve_links(ctx, &text, &mut links).await?;

    // 5. References.
    let text = render_references(&text);

    // 5b. MediaWiki-syntax compatibility (== headings ==, '''bold''', ''italic'').
    let text = wikitext_compat(&text);

    // Restore protected code spans before Markdown.
    let text = restore_code(&text, &protected);

    // 6. Markdown → HTML.
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    opts.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    opts.insert(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION);
    let parser = pulldown_cmark::Parser::new_ext(&text, opts);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);

    // 7. Heading anchors + TOC.
    let (html, toc) = headings_and_toc(&html);

    // 8. Sanitize.
    let html = sanitize(&html);

    Ok(RenderResult { html, categories, links, redirect, toc })
}

/// Convenience: render with default depth from settings.
pub async fn render_page(
    state: &AppState,
    wiki_id: Uuid,
    namespace: &str,
    title: &str,
    source: &str,
) -> Result<RenderResult, WikiError> {
    let ctx = Ctx {
        state,
        wiki_id,
        namespace: namespace.to_string(),
        title: title.to_string(),
        max_depth: state.settings.wiki.max_template_depth,
    };
    render(&ctx, source).await
}

// ── 1. Code protection ──────────────────────────────────────────────────────

fn protect_code(src: &str, store: &mut Vec<String>) -> String {
    thread_local! {
        static FENCE: Regex = Regex::new(r"(?s)```.*?```|`[^`\n]+`|<nowiki>.*?</nowiki>").unwrap();
    }
    FENCE.with(|re| {
        re.replace_all(src, |caps: &regex::Captures| {
            let idx = store.len();
            let mut m = caps[0].to_string();
            // <nowiki> content becomes a plain (escaped) span so it is not reparsed.
            if let Some(inner) = m.strip_prefix("<nowiki>").and_then(|s| s.strip_suffix("</nowiki>")) {
                m = format!("<span class=\"nowiki\">{}</span>", html_escape(inner));
            }
            store.push(m);
            format!("\u{0}KW{idx}\u{0}")
        }).into_owned()
    })
}

fn restore_code(text: &str, store: &[String]) -> String {
    let mut out = text.to_string();
    for (idx, original) in store.iter().enumerate() {
        out = out.replace(&format!("\u{0}KW{idx}\u{0}"), original);
    }
    out
}

// ── 2. Redirect + magic words ───────────────────────────────────────────────

fn extract_redirect(text: &str) -> (String, Option<String>) {
    thread_local! {
        static RE: Regex = Regex::new(r"(?im)^\s*#(?:REDIRECT|REDIRECTION)\s*\[\[([^\]]+)\]\]").unwrap();
    }
    RE.with(|re| {
        if let Some(c) = re.captures(text) {
            let target = c[1].trim().to_string();
            let stripped = re.replace(text, "").into_owned();
            (stripped, Some(target))
        } else {
            (text.to_string(), None)
        }
    })
}

fn substitute_magic_words(ctx: &Ctx<'_>, text: &str) -> String {
    let full = page::prefixed_title(&ctx.namespace, &ctx.title);
    text.replace("{{PAGENAME}}", &ctx.title)
        .replace("{{NAMESPACE}}", &ctx.namespace)
        .replace("{{FULLPAGENAME}}", &full)
}

// ── 3. Template expansion ───────────────────────────────────────────────────

async fn expand_templates(ctx: &Ctx<'_>, mut text: String, depth: u32) -> Result<String, WikiError> {
    if depth >= ctx.max_depth {
        return Ok(text);
    }
    let mut cache: HashMap<String, Option<String>> = HashMap::new();
    let mut iterations = 0usize;

    loop {
        iterations += 1;
        if iterations > ITER_CAP {
            break;
        }
        let Some(close) = text.find("}}") else { break };
        let Some(open) = text[..close].rfind("{{") else { break };
        let inner = text[open + 2..close].to_string();
        let replacement = eval_template(ctx, &inner, depth, &mut cache).await?;
        text = format!("{}{}{}", &text[..open], replacement, &text[close + 2..]);
    }
    Ok(text)
}

async fn eval_template(
    ctx: &Ctx<'_>,
    inner: &str,
    depth: u32,
    cache: &mut HashMap<String, Option<String>>,
) -> Result<String, WikiError> {
    let trimmed = inner.trim();

    // Parser functions: {{#if:…}}, {{#ifeq:…}}, {{#switch:…}}
    if let Some(rest) = trimmed.strip_prefix('#') {
        return Ok(eval_parser_function(rest));
    }

    // Plain template: Name | positional | key=value
    let parts = split_template_args(trimmed);
    let name = parts.first().cloned().unwrap_or_default();
    let name = name.trim();
    if name.is_empty() {
        return Ok(String::new());
    }

    // Build arg map (1-based positional + named).
    let mut args: HashMap<String, String> = HashMap::new();
    let mut pos = 0u32;
    for raw in parts.iter().skip(1) {
        if let Some((k, v)) = raw.split_once('=') {
            args.insert(k.trim().to_string(), v.trim().to_string());
        } else {
            pos += 1;
            args.insert(pos.to_string(), raw.trim().to_string());
        }
    }

    // Load template body (Template namespace) — cached per render.
    let (ns, title) = split_namespace(name);
    let lookup_ns = if ns == "Main" { "Template".to_string() } else { ns };
    let slug = slugify(&title);
    let cache_key = format!("{lookup_ns}:{slug}");

    let body = if let Some(b) = cache.get(&cache_key) {
        b.clone()
    } else {
        let b = load_template_body(ctx, &lookup_ns, &slug).await?;
        cache.insert(cache_key, b.clone());
        b
    };

    match body {
        Some(b) => {
            let substituted = substitute_params(&b, &args);
            // Bound nested template depth.
            Box::pin(expand_templates(ctx, substituted, depth + 1)).await
        }
        None => {
            // Missing template → red link to the Template page.
            let label = page::prefixed_title(&lookup_ns, &title);
            Ok(format!(
                "<a class=\"redlink\" href=\"/wiki/{wid}/page/{ns}/{slug}\" data-ns=\"{ns}\" data-title=\"{t}\">{label}</a>",
                wid = ctx.wiki_id, ns = lookup_ns, slug = slug, t = html_attr(&title), label = html_escape(&label)
            ))
        }
    }
}

async fn load_template_body(ctx: &Ctx<'_>, ns: &str, slug: &str) -> Result<Option<String>, WikiError> {
    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT id, file_id FROM pages \
         WHERE wiki_id = $1 AND namespace = $2 AND slug = $3 AND NOT is_deleted",
    )
    .bind(ctx.wiki_id)
    .bind(ns)
    .bind(slug)
    .fetch_optional(&ctx.state.db)
    .await?;

    let Some((_id, file_id)) = row else { return Ok(None) };

    // storage owner of this wiki
    let storage_owner: Uuid = sqlx::query_scalar("SELECT storage_owner_id FROM wikis WHERE id = $1")
        .bind(ctx.wiki_id)
        .fetch_one(&ctx.state.db)
        .await?;

    let env = content_files::read_page_file(ctx.state, storage_owner, file_id).await?;
    Ok(Some(env.content))
}

/// Substitute `{{{name|default}}}` parameters in a template body.
fn substitute_params(body: &str, args: &HashMap<String, String>) -> String {
    thread_local! {
        static RE: Regex = Regex::new(r"\{\{\{\s*([^{}|]+?)\s*(?:\|([^{}]*))?\}\}\}").unwrap();
    }
    RE.with(|re| {
        re.replace_all(body, |c: &regex::Captures| {
            let key = c[1].trim();
            if let Some(v) = args.get(key) {
                v.clone()
            } else {
                c.get(2).map(|m| m.as_str().to_string()).unwrap_or_default()
            }
        }).into_owned()
    })
}

/// Split template inner text on top-level `|`, respecting nested `{{}}`/`[[]]`.
fn split_template_args(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth_brace = 0i32;
    let mut depth_brack = 0i32;
    let bytes: Vec<char> = inner.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied();
        match (c, next) {
            ('{', Some('{')) => { depth_brace += 1; cur.push('{'); cur.push('{'); i += 2; continue; }
            ('}', Some('}')) => { depth_brace -= 1; cur.push('}'); cur.push('}'); i += 2; continue; }
            ('[', Some('[')) => { depth_brack += 1; cur.push('['); cur.push('['); i += 2; continue; }
            (']', Some(']')) => { depth_brack -= 1; cur.push(']'); cur.push(']'); i += 2; continue; }
            ('|', _) if depth_brace == 0 && depth_brack == 0 => { parts.push(cur.clone()); cur.clear(); i += 1; continue; }
            _ => { cur.push(c); i += 1; }
        }
    }
    parts.push(cur);
    parts
}

fn eval_parser_function(rest: &str) -> String {
    // rest is like "if:cond|then|else" or "ifeq:a|b|then|else" or "switch:v|a=x|b=y|default"
    let Some((name, body)) = rest.split_once(':') else { return String::new() };
    let args = split_template_args(body);
    let arg = |i: usize| args.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
    match name.trim() {
        "if" => {
            if !arg(0).is_empty() { arg(1) } else { arg(2) }
        }
        "ifeq" => {
            if arg(0) == arg(1) { arg(2) } else { arg(3) }
        }
        "switch" => {
            let needle = arg(0);
            let mut default = String::new();
            for part in args.iter().skip(1) {
                if let Some((k, v)) = part.split_once('=') {
                    if k.trim() == needle { return v.trim().to_string(); }
                    if k.trim() == "#default" { default = v.trim().to_string(); }
                } else {
                    default = part.trim().to_string();
                }
            }
            default
        }
        _ => String::new(),
    }
}

// ── 4. Categories + internal links ──────────────────────────────────────────

fn extract_categories(text: &str, out: &mut Vec<CategoryRef>) -> String {
    thread_local! {
        static RE: Regex = Regex::new(r"\[\[\s*(?:Cat[ée]gorie|Category)\s*:\s*([^\]\|]+?)\s*(?:\|[^\]]*)?\]\]").unwrap();
    }
    RE.with(|re| {
        let stripped = re.replace_all(text, |c: &regex::Captures| {
            let title = page::normalize_title(&c[1]);
            let slug = slugify(&title);
            if !out.iter().any(|x| x.slug == slug) {
                out.push(CategoryRef { title, slug });
            }
            ""
        });
        stripped.into_owned()
    })
}

async fn resolve_links(ctx: &Ctx<'_>, text: &str, out: &mut Vec<LinkRef>) -> Result<String, WikiError> {
    thread_local! {
        static RE: Regex = Regex::new(r"\[\[\s*([^\]\|]+?)\s*(?:\|\s*([^\]]*?)\s*)?\]\]").unwrap();
    }
    // Collect matches first (regex closure can't be async).
    let matches: Vec<(String, String, Option<String>)> = RE.with(|re| {
        re.captures_iter(text)
            .map(|c| (c[0].to_string(), c[1].to_string(), c.get(2).map(|m| m.as_str().to_string())))
            .collect()
    });

    let mut replacements: HashMap<String, String> = HashMap::new();
    for (whole, target_raw, label_opt) in matches {
        if replacements.contains_key(&whole) {
            continue;
        }
        // Leading ':' forces a normal link even for File/Category.
        let forced = target_raw.trim().strip_prefix(':').map(|s| s.to_string());
        let target = forced.clone().unwrap_or_else(|| target_raw.clone());
        let (ns, title) = split_namespace(&target);

        // File: embed as image when not forced into a plain link.
        if ns == "File" && forced.is_none() {
            let alt = label_opt.clone().unwrap_or_else(|| title.clone());
            let html = format!(
                "<span class=\"wiki-file\" data-ns=\"File\" data-title=\"{t}\">[{label}]</span>",
                t = html_attr(&title), label = html_escape(&alt)
            );
            replacements.insert(whole, html);
            continue;
        }

        let slug = slugify(&title);
        let label = label_opt.unwrap_or_else(|| page::prefixed_title(&ns, &title));

        // Record link for the graph (skip self).
        if !out.iter().any(|l| l.namespace == ns && l.slug == slug) {
            out.push(LinkRef { namespace: ns.clone(), title: title.clone(), slug: slug.clone() });
        }

        let exists = page_exists(ctx, &ns, &slug).await?;
        let class = if exists { "wikilink" } else { "redlink" };
        let html = format!(
            "<a class=\"{class}\" href=\"/wiki/{wid}/page/{ns}/{slug}\" data-ns=\"{ns}\" data-title=\"{t}\">{label}</a>",
            class = class, wid = ctx.wiki_id, ns = ns, slug = slug,
            t = html_attr(&title), label = html_escape(&label)
        );
        replacements.insert(whole, html);
    }

    let mut result = text.to_string();
    for (k, v) in replacements {
        result = result.replace(&k, &v);
    }
    Ok(result)
}

async fn page_exists(ctx: &Ctx<'_>, ns: &str, slug: &str) -> Result<bool, WikiError> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages WHERE wiki_id = $1 AND namespace = $2 AND slug = $3 AND NOT is_deleted",
    )
    .bind(ctx.wiki_id)
    .bind(ns)
    .bind(slug)
    .fetch_one(&ctx.state.db)
    .await?;
    Ok(n > 0)
}

// ── 5. References ───────────────────────────────────────────────────────────

fn render_references(text: &str) -> String {
    thread_local! {
        static RE: Regex = Regex::new(r"(?s)<ref>(.*?)</ref>").unwrap();
    }
    RE.with(|re| {
        let mut notes: Vec<String> = Vec::new();
        let body = re.replace_all(text, |c: &regex::Captures| {
            notes.push(c[1].trim().to_string());
            let n = notes.len();
            format!("<sup class=\"ref\" id=\"ref-{n}\"><a href=\"#fn-{n}\">[{n}]</a></sup>")
        }).into_owned();

        if notes.is_empty() {
            return body;
        }
        let mut list = String::from("\n\n<ol class=\"references\">\n");
        for (i, note) in notes.iter().enumerate() {
            let n = i + 1;
            list.push_str(&format!("<li id=\"fn-{n}\">{}</li>\n", html_escape(note)));
        }
        list.push_str("</ol>\n");
        // Replace an explicit <references/> marker, else append.
        if body.contains("<references/>") || body.contains("<references />") {
            body.replace("<references/>", &list).replace("<references />", &list)
        } else {
            format!("{body}{list}")
        }
    })
}

// ── 5b. MediaWiki syntax compatibility ──────────────────────────────────────

fn wikitext_compat(text: &str) -> String {
    thread_local! {
        // `== Heading ==` (1–6 `=`) on its own line → ATX markdown heading.
        static HEAD: Regex = Regex::new(r"(?m)^[ \t]*(={1,6})[ \t]*(.+?)[ \t]*=+[ \t]*$").unwrap();
        static BOLDIT: Regex = Regex::new(r"'''''(.+?)'''''").unwrap();
        static BOLD: Regex = Regex::new(r"'''(.+?)'''").unwrap();
        static ITAL: Regex = Regex::new(r"''(.+?)''").unwrap();
    }
    let t = HEAD.with(|re| {
        re.replace_all(text, |c: &regex::Captures| {
            let level = c[1].len().min(6);
            format!("{} {}", "#".repeat(level), c[2].trim())
        }).into_owned()
    });
    let t = BOLDIT.with(|re| re.replace_all(&t, "***$1***").into_owned());
    let t = BOLD.with(|re| re.replace_all(&t, "**$1**").into_owned());
    ITAL.with(|re| re.replace_all(&t, "*$1*").into_owned())
}

// ── 7. Heading anchors + TOC ────────────────────────────────────────────────

fn headings_and_toc(html: &str) -> (String, Vec<TocEntry>) {
    thread_local! {
        static RE: Regex = Regex::new(r"<h([2-4])>(.*?)</h[2-4]>").unwrap();
    }
    RE.with(|re| {
        let mut toc: Vec<TocEntry> = Vec::new();
        let mut seen: HashMap<String, u32> = HashMap::new();
        let out = re.replace_all(html, |c: &regex::Captures| {
            let level: u8 = c[1].parse().unwrap_or(2);
            let inner = &c[2];
            let text = strip_tags(inner);
            let mut id = slugify(&text);
            if id.is_empty() { id = "section".to_string(); }
            let counter = seen.entry(id.clone()).or_insert(0);
            *counter += 1;
            if *counter > 1 {
                id = format!("{id}-{counter}");
            }
            toc.push(TocEntry { level, text: text.clone(), id: id.clone() });
            format!("<h{level} id=\"{id}\">{inner}</h{level}>", level = level, id = id, inner = inner)
        }).into_owned();
        (out, toc)
    })
}

// ── 8. Sanitization ─────────────────────────────────────────────────────────

fn sanitize(html: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(["sup", "section"])
        .add_generic_attributes(["class", "id"])
        .add_generic_attribute_prefixes(["data-"])
        .url_relative(ammonia::UrlRelative::PassThrough);
    builder.clean(html).to_string()
}

// ── small helpers ───────────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn html_attr(s: &str) -> String {
    html_escape(s).replace('"', "&quot;")
}

fn strip_tags(s: &str) -> String {
    thread_local! {
        static RE: Regex = Regex::new(r"<[^>]+>").unwrap();
    }
    RE.with(|re| re.replace_all(s, "").trim().to_string())
}

/// Re-export so namespace validation has a single source of truth.
pub fn validate_namespace(raw: &str) -> Option<&'static str> {
    canonical_namespace(raw)
}
