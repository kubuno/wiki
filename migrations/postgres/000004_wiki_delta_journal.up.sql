-- Move the delta layer off PostgreSQL sequences + triggers and onto the
-- application-driven `kubuno_db::journal` primitive (one shared counter row per
-- domain, seqs taken in Rust at write time, tombstones written in the same
-- transaction). Neither the sequence nor the trigger mechanism has a portable
-- form on MySQL/SQLite, so it is retired here on PostgreSQL too; the tombstone
-- TABLE keeps its exact shape (no data migration), only its triggers go.
--
-- The `change_seq` columns stay `BIGINT NOT NULL`, but their DEFAULT switches
-- from `nextval(...)` to `0`: the application now supplies every value. The
-- `wikis_updated_at` / `pages_updated_at` triggers from 000001 are deliberately
-- LEFT in place (their MySQL/SQLite equivalents are handled in Rust / by those
-- engines' migrations); only the change-seq and tombstone machinery is removed.

-- ── wikis: BEFORE UPDATE seq, AFTER DELETE tombstone, member child bump ───────
DROP TRIGGER IF EXISTS trg_wiki_change_seq  ON wikis;
DROP TRIGGER IF EXISTS trg_wiki_tombstone   ON wikis;
DROP TRIGGER IF EXISTS trg_member_bump_wiki ON wiki_members;
DROP FUNCTION IF EXISTS wiki_bump_change_seq();
DROP FUNCTION IF EXISTS wiki_tombstone();
DROP FUNCTION IF EXISTS member_bump_wiki();

-- ── pages: BEFORE UPDATE seq ─────────────────────────────────────────────────
DROP TRIGGER IF EXISTS trg_page_change_seq ON pages;
DROP FUNCTION IF EXISTS page_bump_change_seq();

-- The DEFAULT references the sequence, so it must go before the sequence does.
ALTER TABLE wikis ALTER COLUMN change_seq SET DEFAULT 0;
ALTER TABLE pages ALTER COLUMN change_seq SET DEFAULT 0;
DROP SEQUENCE IF EXISTS wiki_change_seq;
DROP SEQUENCE IF EXISTS page_change_seq;

-- ── The journal's shared counter, seeded to continue the existing sequences ───
CREATE TABLE IF NOT EXISTS change_counter (
    domain VARCHAR(190) NOT NULL PRIMARY KEY,
    n      BIGINT       NOT NULL
);

-- Seed each domain to the current max so `next_seq` (n := n + 1) never hands out
-- a value an existing row already holds.
INSERT INTO change_counter (domain, n)
    SELECT 'wikis', COALESCE(MAX(change_seq), 0) FROM wikis
    ON CONFLICT (domain) DO NOTHING;
INSERT INTO change_counter (domain, n)
    SELECT 'pages', COALESCE(MAX(change_seq), 0) FROM pages
    ON CONFLICT (domain) DO NOTHING;

-- The tombstone table (wiki_tombstones) keeps its 000002 shape unchanged; only
-- its trigger was dropped above.
