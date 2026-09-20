-- SQLite — `wiki` is an ATTACHed database file, attached on every pooled
-- connection by kubuno-db, so the qualified names below resolve as they do on
-- the other two engines. This single file declares the FINAL shape the
-- PostgreSQL side reached across its 000001..000004 migrations.
--
-- Differences from PostgreSQL, and why:
--   * UUID -> BLOB, TIMESTAMPTZ -> TEXT (`%F %T%.f`, UTC), as sqlx encodes them.
--   * No DEFAULT on `id`: SQLite has no UUID generator; the process supplies it.
--   * Full-text search is the normalized-column form (title_norm / body_norm,
--     filled in Rust): no tsvector, no unaccent.
--   * The delta layer is the journal (change_counter + per-row change_seq) plus
--     the wiki tombstone table; no sequences, no triggers.
--   * Foreign-key REFERENCES are unqualified (SQLite assumes the same database);
--     kubuno-db enables `PRAGMA foreign_keys`, so CASCADE deletes fire.

CREATE TABLE wiki.wikis (
    id               BLOB    NOT NULL PRIMARY KEY,
    owner_id         BLOB    NOT NULL,
    storage_owner_id BLOB    NOT NULL,
    slug             TEXT    NOT NULL,
    name             TEXT    NOT NULL,
    description      TEXT    NOT NULL,
    is_shared        INTEGER NOT NULL DEFAULT 0,
    change_seq       INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    updated_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    UNIQUE (owner_id, slug)
);
CREATE INDEX wiki.idx_wikis_change_seq ON wikis(owner_id, change_seq);

CREATE TABLE wiki.wiki_members (
    wiki_id  BLOB NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    user_id  BLOB NOT NULL,
    role     TEXT NOT NULL DEFAULT 'editor'
                 CHECK (role IN ('admin', 'editor', 'reader')),
    added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    PRIMARY KEY (wiki_id, user_id)
);

CREATE TABLE wiki.pages (
    id                BLOB    NOT NULL PRIMARY KEY,
    wiki_id           BLOB    NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    namespace         TEXT    NOT NULL DEFAULT 'Main',
    title             TEXT    NOT NULL,
    slug              TEXT    NOT NULL,
    file_id           BLOB    NOT NULL,
    redirect_to       TEXT,
    preview           TEXT    NOT NULL,
    byte_size         INTEGER NOT NULL DEFAULT 0,
    current_author_id BLOB,
    current_rev_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    is_deleted        INTEGER NOT NULL DEFAULT 0,
    title_norm        TEXT    NOT NULL,
    body_norm         TEXT    NOT NULL,
    change_seq        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    UNIQUE (wiki_id, namespace, slug)
);
CREATE INDEX wiki.idx_pages_wiki       ON pages(wiki_id);
CREATE INDEX wiki.idx_pages_file       ON pages(file_id);
CREATE INDEX wiki.idx_pages_change_seq ON pages(wiki_id, change_seq);
CREATE INDEX wiki.idx_pages_title_norm ON pages(title_norm);
CREATE INDEX wiki.idx_pages_body_norm  ON pages(body_norm);

CREATE TABLE wiki.page_links (
    source_page_id   BLOB NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    wiki_id          BLOB NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    target_namespace TEXT NOT NULL DEFAULT 'Main',
    target_title     TEXT NOT NULL,
    target_slug      TEXT NOT NULL,
    target_page_id   BLOB,
    PRIMARY KEY (source_page_id, target_namespace, target_slug)
);
CREATE INDEX wiki.idx_links_target ON page_links(target_page_id);
CREATE INDEX wiki.idx_links_wanted ON page_links(wiki_id, target_namespace, target_slug);

CREATE TABLE wiki.page_categories (
    page_id        BLOB NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    wiki_id        BLOB NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    category_title TEXT NOT NULL,
    category_slug  TEXT NOT NULL,
    PRIMARY KEY (page_id, category_slug)
);
CREATE INDEX wiki.idx_pagecats_cat ON page_categories(wiki_id, category_slug);

CREATE TABLE wiki.recent_changes (
    id          BLOB    NOT NULL PRIMARY KEY,
    wiki_id     BLOB    NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    page_id     BLOB,
    namespace   TEXT    NOT NULL DEFAULT 'Main',
    title       TEXT    NOT NULL,
    author_id   BLOB,
    author_name TEXT    NOT NULL DEFAULT '',
    comment     TEXT    NOT NULL,
    minor       INTEGER NOT NULL DEFAULT 0,
    change_type TEXT    NOT NULL DEFAULT 'edit'
                    CHECK (change_type IN ('create', 'edit', 'delete', 'move')),
    byte_delta  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX wiki.idx_rc_wiki ON recent_changes(wiki_id, created_at);

-- ── Delta journal (portable change layer) ────────────────────────────────────
CREATE TABLE wiki.change_counter (
    domain TEXT   NOT NULL PRIMARY KEY,
    n      BIGINT NOT NULL
);

CREATE TABLE wiki.wiki_tombstones (
    id         BLOB    NOT NULL PRIMARY KEY,
    owner_id   BLOB    NOT NULL,
    change_seq BIGINT  NOT NULL,
    deleted_at TEXT    NOT NULL
);
CREATE INDEX wiki.idx_wiki_tomb ON wiki_tombstones(owner_id, change_seq);
