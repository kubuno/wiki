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
- **Local-first delta sync** — `GET /wikis/delta` and `GET /pages/delta` stream owner-scoped
  changes past a monotonic cursor: wiki changes inline their members (with tombstones for
  hard deletes), page changes inline the full `.kbwik` envelope and category rows, so a
  local-first client can mirror a personal wiki and read it offline. Creation endpoints
  accept client-minted UUIDs so offline-created wikis and pages replay verbatim.

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

## Install

This module ships in the **all-in-one [Kubuno](https://github.com/kubuno/core) Docker image** (`ghcr.io/kubuno/kubuno`) — the easiest way to self-host a full Kubuno instance (core + every module). See **[kubuno/docker](https://github.com/kubuno/docker)** for `docker compose` instructions.

To build this module from source (Debian package), see below.

## Development

```bash
cargo build --release                     # backend (shared crates from git tags)
cd frontend && npm ci && npm run build     # frontend bundle
bash build_deb.sh --install                # build + install the .deb locally
bash ../_tools/deploy_local.sh wiki         # fast local rebuild + restart
```

Native packages for other platforms are built the same way — each one lays the module out
exactly like the `.deb`, so the core discovers it identically:

```bash
bash build_rpm.sh        # Fedora / RHEL / openSUSE  → dist/kubuno-wiki-*.rpm
bash build_windows.sh    # Windows installer (NSIS)  → dist/kubuno-wiki-setup-*-x64.exe
bash build_macos.sh      # macOS (run on a Mac)      → dist/kubuno-wiki-*.pkg
```

All of these are also produced by CI and attached to the GitHub Release on every `v*` tag.

## License

[AGPL-3.0-or-later](LICENSE) © Kubuno contributors.
