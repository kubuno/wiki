-- MySQL / MariaDB — the `wiki` database is created by kubuno-db's schema setup
-- before the migrator runs, so there is no CREATE DATABASE here. This single
-- file declares the FINAL shape the PostgreSQL side reached across its
-- 000001..000004 migrations (delta journal + normalized search columns).
--
-- Differences from PostgreSQL, and why:
--   * UUID -> BINARY(16): what sqlx encodes a `uuid::Uuid` as on MySQL.
--   * No DEFAULT on `id`: MySQL has no gen_random_uuid() and no RETURNING, so
--     the process supplies every primary key.
--   * TIMESTAMPTZ -> DATETIME(6); every value written is UTC (the pool pins
--     `time_zone = '+00:00'`).
--   * updated_at is maintained by ON UPDATE CURRENT_TIMESTAMP(6).
--   * Full-text search is the normalized-column form (title_norm / body_norm,
--     filled in Rust): no tsvector, no GIN, no unaccent.
--   * The delta layer is the journal (change_counter + per-row change_seq) plus
--     the wiki tombstone table; no sequences, no triggers.
--   * Partial indexes (WHERE ...) become plain indexes (MySQL has none).
--   * utf8mb4_bin so a UNIQUE key stays case- and accent-sensitive.

CREATE TABLE wikis (
    id               BINARY(16)   NOT NULL PRIMARY KEY,
    owner_id         BINARY(16)   NOT NULL,
    storage_owner_id BINARY(16)   NOT NULL,
    slug             VARCHAR(120) NOT NULL,
    name             VARCHAR(200) NOT NULL,
    description      TEXT         NOT NULL,
    is_shared        BOOLEAN      NOT NULL DEFAULT FALSE,
    change_seq       BIGINT       NOT NULL DEFAULT 0,
    created_at       DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at       DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
                                  ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE (owner_id, slug)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_wikis_change_seq ON wikis(owner_id, change_seq);

CREATE TABLE wiki_members (
    wiki_id  BINARY(16)  NOT NULL,
    user_id  BINARY(16)  NOT NULL,
    role     VARCHAR(20) NOT NULL DEFAULT 'editor'
                 CHECK (role IN ('admin', 'editor', 'reader')),
    added_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (wiki_id, user_id),
    FOREIGN KEY (wiki_id) REFERENCES wikis(id) ON DELETE CASCADE
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE pages (
    id                BINARY(16)   NOT NULL PRIMARY KEY,
    wiki_id           BINARY(16)   NOT NULL,
    namespace         VARCHAR(40)  NOT NULL DEFAULT 'Main',
    title             VARCHAR(500) NOT NULL,
    slug              VARCHAR(560) NOT NULL,
    file_id           BINARY(16)   NOT NULL,
    redirect_to       VARCHAR(560) NULL,
    preview           TEXT         NOT NULL,
    byte_size         INT          NOT NULL DEFAULT 0,
    current_author_id BINARY(16)   NULL,
    current_rev_at    DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    is_deleted        BOOLEAN      NOT NULL DEFAULT FALSE,
    title_norm        TEXT         NOT NULL,
    body_norm         TEXT         NOT NULL,
    change_seq        BIGINT       NOT NULL DEFAULT 0,
    created_at        DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at        DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
                                   ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE (wiki_id, namespace, slug),
    FOREIGN KEY (wiki_id) REFERENCES wikis(id) ON DELETE CASCADE
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_pages_wiki        ON pages(wiki_id);
CREATE INDEX idx_pages_file        ON pages(file_id);
CREATE INDEX idx_pages_change_seq  ON pages(wiki_id, change_seq);
CREATE INDEX idx_pages_title_norm  ON pages(title_norm(191));
CREATE INDEX idx_pages_body_norm   ON pages(body_norm(191));

CREATE TABLE page_links (
    source_page_id   BINARY(16)   NOT NULL,
    wiki_id          BINARY(16)   NOT NULL,
    target_namespace VARCHAR(40)  NOT NULL DEFAULT 'Main',
    target_title     VARCHAR(500) NOT NULL,
    target_slug      VARCHAR(560) NOT NULL,
    target_page_id   BINARY(16)   NULL,
    PRIMARY KEY (source_page_id, target_namespace, target_slug),
    FOREIGN KEY (source_page_id) REFERENCES pages(id) ON DELETE CASCADE,
    FOREIGN KEY (wiki_id)        REFERENCES wikis(id) ON DELETE CASCADE
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_links_target ON page_links(target_page_id);
CREATE INDEX idx_links_wanted ON page_links(wiki_id, target_namespace, target_slug);

CREATE TABLE page_categories (
    page_id        BINARY(16)   NOT NULL,
    wiki_id        BINARY(16)   NOT NULL,
    category_title VARCHAR(500) NOT NULL,
    category_slug  VARCHAR(560) NOT NULL,
    PRIMARY KEY (page_id, category_slug),
    FOREIGN KEY (page_id) REFERENCES pages(id) ON DELETE CASCADE,
    FOREIGN KEY (wiki_id) REFERENCES wikis(id) ON DELETE CASCADE
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_pagecats_cat ON page_categories(wiki_id, category_slug);

CREATE TABLE recent_changes (
    id          BINARY(16)   NOT NULL PRIMARY KEY,
    wiki_id     BINARY(16)   NOT NULL,
    page_id     BINARY(16)   NULL,
    namespace   VARCHAR(40)  NOT NULL DEFAULT 'Main',
    title       VARCHAR(500) NOT NULL,
    author_id   BINARY(16)   NULL,
    author_name VARCHAR(200) NOT NULL DEFAULT '',
    comment     TEXT         NOT NULL,
    minor       BOOLEAN      NOT NULL DEFAULT FALSE,
    change_type VARCHAR(20)  NOT NULL DEFAULT 'edit'
                    CHECK (change_type IN ('create', 'edit', 'delete', 'move')),
    byte_delta  INT          NOT NULL DEFAULT 0,
    created_at  DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    FOREIGN KEY (wiki_id) REFERENCES wikis(id) ON DELETE CASCADE
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_rc_wiki ON recent_changes(wiki_id, created_at);

-- ── Delta journal (portable change layer) ────────────────────────────────────
CREATE TABLE change_counter (
    domain VARCHAR(190) NOT NULL PRIMARY KEY,
    n      BIGINT       NOT NULL
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE wiki_tombstones (
    id         BINARY(16)  NOT NULL PRIMARY KEY,
    owner_id   BINARY(16)  NOT NULL,
    change_seq BIGINT      NOT NULL,
    deleted_at DATETIME(6) NOT NULL
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_wiki_tomb ON wiki_tombstones(owner_id, change_seq);
