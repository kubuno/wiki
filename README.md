<!--
  SPDX-FileCopyrightText: 2026 Kubuno contributors
  SPDX-License-Identifier: AGPL-3.0-or-later
-->

<div align="center">

<img src=".github/logo.png" alt="Kubuno Wiki logo" width="120">

# Kubuno — Wiki

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-edition_2021-orange.svg)
![React](https://img.shields.io/badge/React-19-61dafb.svg)
![Status](https://img.shields.io/badge/status-alpha-yellow.svg)
![Module](https://img.shields.io/badge/Kubuno-module-4D38DB.svg)

**A collaborative wiki for Kubuno — write in familiar wikitext, transclude templates, browse namespaces and special pages, and keep every page as a portable file in your own Drive.**

Wiki is a module for [Kubuno](https://github.com/kubuno/core), the self-hosted, libre (AGPLv3) cloud platform — a sovereign alternative to Google Workspace and Microsoft 365. Pages are stored as self-contained **`.kbwik` files** in your Kubuno Drive — the database is only an index — so a wiki is yours to keep, move and back up like any other document.

</div>

---

## ✨ Features

- 📚 **Personal & shared wikis** — keep a private knowledge base in your own Drive, or open a shared, collaborative wiki with per-member roles (`admin` / `editor` / `reader`).
- 🖋️ **Extended Markdown + wikitext** — CommonMark (tables, footnotes, task lists…) alongside classic wiki conventions: `[[internal links]]`, `== headings ==`, `'''bold'''` / `''italic''`, `<ref>` references, `#REDIRECT`, and an automatic table of contents.
- 🧩 **Templates & transclusion** — `{{Template|positional|named=value}}` with `{{{1|default}}}` parameters, the `{{#if}}`, `{{#ifeq}}` and `{{#switch}}` parser functions, and magic words (`{{PAGENAME}}`, `{{NAMESPACE}}`, `{{FULLPAGENAME}}`), bounded by a configurable transclusion depth.
- 🗂️ **Namespaces & talk pages** — `Main`, `Talk`, `User`, `Wiki`, `Template`, `Category`, `File` and `Help` (with French aliases).
- 🏷️ **Categories & special pages** — `[[Category:…]]` membership plus *All pages*, *Recent changes*, *Wanted pages* (red links), *Orphaned pages* and *Categories*.
- 🔗 **Links & navigation** — red links for missing pages, "what links here" backlinks, redirects, and full-text search (French-aware, accent-insensitive).
- 🕔 **Revision history** — every save is recorded inside the `.kbwik` file and browsable in the history viewer; an administrator can cap how many revisions each page keeps.
- 🔄 **Local-first delta sync** — `GET /wikis/delta` and `GET /pages/delta` stream owner-scoped changes past a monotonic cursor (member rows and full `.kbwik` envelopes, with tombstones for hard deletes), and creation endpoints accept client-minted UUIDs, so an offline-capable client can mirror a personal wiki and replay verbatim.
- 🎛️ **Administrable by policy** — control who may create a personal or a shared wiki, and cap the maximum page source size.

## 🏗️ Architecture

Like every Kubuno app, Wiki is an **independent process**, not a library linked into the core. It registers with the [core](https://github.com/kubuno/core) at startup; the core then proxies its routes (`/api/v1/wiki/*`), distributes platform events to it, serves its runtime-loaded React frontend bundle and manages its lifecycle.

- **Port** — the backend listens on `127.0.0.1:3120` and is reached only through the core's reverse proxy.
- **Backend** — `src/`: Axum + SQLx over PostgreSQL, confined to the `wiki` schema (an index only); migrations in `migrations/`. Page content lives in `.kbwik` files (`application/vnd.kubuno.wiki+json`) stored through the Drive module; shared wikis are owned by a reserved system user. The rendering pipeline protects code, expands templates and magic words, resolves categories and links, renders Markdown, builds the TOC and sanitises the result.
- **Frontend** — `frontend/`: a React 19 bundle built to `entry.js` + `entry.css`, consuming `@kubuno/sdk`, `@ui` (`@kubuno/ui`) and `@kubuno/drive`. At runtime those specifiers are `external` and resolved by the host's import map to its single shared instances; the npm packages are used only for building and type-checking.
- **Trust boundary** — proxied requests are authenticated from a signed `X-Kubuno-Auth` token minted by the core (see `kubuno-modauth`), never from plain `X-Kubuno-User-*` headers.

## 📦 Install

The easiest way to self-host a full Kubuno instance (core + every module) is the **all-in-one Docker image** (`ghcr.io/kubuno/kubuno`), which already bundles this module — see **[kubuno/docker](https://github.com/kubuno/docker)** for `docker compose` instructions.

To add the module to an existing instance, install its **`.kbpkg`** — the single, cross-platform package format a Kubuno server unpacks by itself (no `.deb`/`.rpm`/`.exe`/`.pkg`, and no external tools). Each tagged release (`v*`) attaches a Linux `.kbpkg` (built by `build.yml`) and Windows/macOS `.kbpkg` files (built by `dist.yml`) to its [GitHub Release](https://github.com/kubuno/wiki/releases):

```bash
# From the admin console: Modules → Install, then drop the .kbpkg — or, offline, from the CLI:
sudo kubuno modules:install dist/wiki-<version>-<os>-<arch>.kbpkg
sudo systemctl restart kubuno     # the core loads the module on (re)start
```

## 🛠️ Build & development

**Requirements:** Rust ≥ 1.82, Node.js ≥ 24, PostgreSQL 16. No `kubuno/core` checkout is needed — shared Rust crates come from tagged git dependencies, and the `@kubuno/*` frontend libraries from the public npm scope.

```bash
cargo build --release                     # → target/release/kubuno-wiki
cd frontend && npm ci && npm run build     # → dist/{entry.js, entry.css}

bash build_kbpkg.sh                        # → dist/wiki-<version>-<os>-<arch>.kbpkg
bash build_kbpkg.sh --install              # build, install into the module store, restart
```

Once the module has been installed at least once, iterate quickly without repackaging:

```bash
bash ../_tools/deploy_local.sh wiki             # backend + frontend
bash ../_tools/deploy_local.sh wiki --frontend  # frontend only (fastest)
```

## 📦 Tech stack

Rust 2021 · Axum 0.7 · Tokio · SQLx 0.8 (PostgreSQL 16, schema `wiki`) · `ammonia` HTML sanitisation · `.kbwik` files via Drive — React 19 · TypeScript · Vite · Tailwind CSS v4 · Zustand · React Query, on the shared `@kubuno/sdk`, `@ui` and `@kubuno/drive` surfaces.

## 🤝 Contributing

Contributions are welcome. Please open an issue to discuss any significant change before submitting a pull request.

## 📄 License

[AGPL-3.0-or-later](LICENSE) © Kubuno contributors.
