<!--
  SPDX-FileCopyrightText: 2026 Kubuno contributors
  SPDX-License-Identifier: AGPL-3.0-or-later
-->

# Kubuno Wiki

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)

A collaborative, **MediaWiki-inspired** wiki module for [Kubuno](https://github.com/kubuno) —
the self-hosted, libre alternative to Google Workspace and Microsoft 365.

Pages are written in an extended Markdown dialect with familiar wiki conventions
(`[[internal links]]`, `{{Templates}}`, `[[Category:…]]`, `#REDIRECT`, `== headings ==`,
`'''bold'''`) and are stored as portable **`.kbwik` files** in your Kubuno Drive — the
database is only an index. Each `.kbwik` file is self-contained and carries the page source,
a rendered HTML cache and the full revision history.

## Features

- **Personal & shared wikis** — keep a private knowledge base in your own Drive, or create a
  shared, collaborative wiki with per-member roles (`admin` / `editor` / `reader`).
- **Extended Markdown + wikitext** — CommonMark (tables, footnotes, task lists, …) plus
  `[[links]]`, `== headings ==`, `'''bold'''` / `''italic''`, `<ref>` references and a
  table of contents.
- **Templates & transclusion** — `{{Template|positional|named=value}}` with `{{{1|default}}}`
  parameters, the `{{#if}}`, `{{#ifeq}}`, `{{#switch}}` parser functions and magic words
  (`{{PAGENAME}}`, `{{NAMESPACE}}`, `{{FULLPAGENAME}}`).
- **Namespaces & talk pages** — `Main`, `Talk`, `User`, `Wiki`, `Template`, `Category`,
  `File`, `Help` (with French aliases).
- **Categories & special pages** — `[[Category:…]]` membership plus *All pages*,
  *Recent changes*, *Wanted pages* (red links), *Orphaned pages* and *Categories*.
- **Links & navigation** — red links for missing pages, "what links here" backlinks,
  redirects, full-text search (French + unaccent).
- **Revision history** — every save is recorded inside the `.kbwik` file (history viewer).

## Architecture

The module is an independent process that registers with the Kubuno core on start-up; the
core proxies its routes and forwards the authenticated user via `x-kubuno-user-*` headers.

| | |
|---|---|
| Port | `3120` |
| PostgreSQL schema | `wiki` (index only) |
| Storage | `.kbwik` files via the `drive` module (`application/vnd.kubuno.wiki+json`) |
| Shared-wiki storage owner | reserved system user (`Uuid::from_u128(1)`) |

### Backend (Rust · Axum · SQLx)

- `services/wiki_markup.rs` — the rendering pipeline (protect code → redirect/magic words →
  template expansion → categories/links → references → wikitext compatibility → Markdown →
  TOC → `ammonia` sanitisation).
- `services/content_files.rs` — `.kbwik` read/write (gzipped JSON envelope + revisions).
- `services/page_service.rs` — page CRUD; rebuilds the link graph, categories and the
  recent-changes feed atomically on every save.
- `services/wiki_service.rs` / `permission_service.rs` — wiki lifecycle, membership, roles.

### Frontend (React 19 · TypeScript · Vite)

Loaded at runtime by the host as an ESM bundle (`entry.js` exporting `register()`); shared
specifiers (`react`, `@kubuno/sdk`, `@ui`, …) are resolved by the host import map.

## Development

```bash
cargo build --release                     # backend (shared crates from git tags)
cd frontend && npm ci && npm run build     # frontend bundle
bash build_deb.sh --install                # build + install the .deb locally
bash ../_tools/deploy_local.sh wiki         # fast local rebuild + restart
```

## License

[AGPL-3.0-or-later](LICENSE) © Kubuno contributors.
