-- Kubuno Wiki — main schema (INDEX ONLY; page content lives in .kbwik files).

CREATE OR REPLACE FUNCTION wiki_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

-- ── Wiki spaces (personal or shared) ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS wikis (
    id                UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id          UUID NOT NULL,
    -- Drive owner under which the .kbwik files are stored. Equals owner_id for
    -- personal wikis, or the reserved system user for shared wikis.
    storage_owner_id  UUID NOT NULL,
    slug              VARCHAR(120) NOT NULL,
    name              VARCHAR(200) NOT NULL,
    description       TEXT NOT NULL DEFAULT '',
    is_shared         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (owner_id, slug)
);

CREATE TRIGGER wikis_updated_at BEFORE UPDATE ON wikis
    FOR EACH ROW EXECUTE FUNCTION wiki_set_updated_at();

-- ── Members of shared wikis (role-based access) ──────────────────────────────
CREATE TABLE IF NOT EXISTS wiki_members (
    wiki_id    UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL,
    role       VARCHAR(20) NOT NULL DEFAULT 'editor'
                   CHECK (role IN ('admin', 'editor', 'reader')),
    added_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (wiki_id, user_id)
);

-- ── Pages (index row; source + html + revisions live in the .kbwik file) ─────
CREATE TABLE IF NOT EXISTS pages (
    id                UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wiki_id           UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    namespace         VARCHAR(40) NOT NULL DEFAULT 'Main',
    title             VARCHAR(500) NOT NULL,
    slug              VARCHAR(560) NOT NULL,
    file_id           UUID NOT NULL,
    redirect_to       VARCHAR(560),
    preview           TEXT NOT NULL DEFAULT '',
    byte_size         INTEGER NOT NULL DEFAULT 0,
    current_author_id UUID,
    current_rev_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_deleted        BOOLEAN NOT NULL DEFAULT FALSE,
    search_vector     TSVECTOR,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wiki_id, namespace, slug)
);

CREATE INDEX IF NOT EXISTS idx_pages_wiki        ON pages(wiki_id) WHERE NOT is_deleted;
CREATE INDEX IF NOT EXISTS idx_pages_search      ON pages USING GIN(search_vector);
CREATE INDEX IF NOT EXISTS idx_pages_file        ON pages(file_id);

CREATE OR REPLACE FUNCTION update_pages_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('french', unaccent(COALESCE(NEW.title, ''))),   'A') ||
        setweight(to_tsvector('french', unaccent(COALESCE(NEW.preview, ''))), 'B');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER pages_search_vector
    BEFORE INSERT OR UPDATE OF title, preview ON pages
    FOR EACH ROW EXECUTE FUNCTION update_pages_search_vector();

CREATE TRIGGER pages_updated_at BEFORE UPDATE ON pages
    FOR EACH ROW EXECUTE FUNCTION wiki_set_updated_at();

-- ── Internal link graph (wikilinks). Unresolved target ⇒ "wanted page". ──────
CREATE TABLE IF NOT EXISTS page_links (
    source_page_id    UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    wiki_id           UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    target_namespace  VARCHAR(40) NOT NULL DEFAULT 'Main',
    target_title      VARCHAR(500) NOT NULL,
    target_slug       VARCHAR(560) NOT NULL,
    target_page_id    UUID,
    PRIMARY KEY (source_page_id, target_namespace, target_slug)
);

CREATE INDEX IF NOT EXISTS idx_links_target  ON page_links(target_page_id);
CREATE INDEX IF NOT EXISTS idx_links_wanted  ON page_links(wiki_id, target_namespace, target_slug) WHERE target_page_id IS NULL;

-- ── Category membership ([[Category:X]] on pages). ───────────────────────────
CREATE TABLE IF NOT EXISTS page_categories (
    page_id         UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    wiki_id         UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    category_title  VARCHAR(500) NOT NULL,
    category_slug   VARCHAR(560) NOT NULL,
    PRIMARY KEY (page_id, category_slug)
);

CREATE INDEX IF NOT EXISTS idx_pagecats_cat ON page_categories(wiki_id, category_slug);

-- ── Recent changes feed (one row per edit; revision bodies stay in files). ───
CREATE TABLE IF NOT EXISTS recent_changes (
    id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wiki_id      UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    page_id      UUID,
    namespace    VARCHAR(40) NOT NULL DEFAULT 'Main',
    title        VARCHAR(500) NOT NULL,
    author_id    UUID,
    author_name  VARCHAR(200) NOT NULL DEFAULT '',
    comment      TEXT NOT NULL DEFAULT '',
    minor        BOOLEAN NOT NULL DEFAULT FALSE,
    change_type  VARCHAR(20) NOT NULL DEFAULT 'edit'
                     CHECK (change_type IN ('create', 'edit', 'delete', 'move')),
    byte_delta   INTEGER NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_rc_wiki ON recent_changes(wiki_id, created_at DESC);
