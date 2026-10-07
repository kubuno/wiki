-- Move full-text search off PostgreSQL's `tsvector` / `to_tsvector` / `ts_rank`
-- and onto `kubuno_db::search`: text is reduced to Snowball French stems and
-- deaccented IN RUST at write time and stored in plain `TEXT` columns, then a
-- query is put through the same reduction and matched with a portable `LIKE`.
-- Because the stemming happens before any SQL, the stored and searched tokens
-- are byte-for-byte identical on PostgreSQL, MySQL and SQLite. The MySQL and
-- SQLite migrations declare the `*_norm` columns from their CREATE TABLE; here
-- the PostgreSQL table sheds its `tsvector` machinery and gains the columns.
--
-- NOTE: the `unaccent` / `pg_trgm` extensions are NOT dropped — the wiki never
-- created them (they are provided by the core's own migrations and shared by
-- other modules); dropping them here would break the rest of the platform.
--
-- What is lost: `pg_trgm`'s typo tolerance (a `LIKE '%stem%'` needs the stem to
-- appear as a substring). Stemming still folds inflections and the normalizer
-- folds accents, so inflected and accented queries still match.

-- Retire the tsvector column, its GIN index and the trigger/function that fed it.
DROP TRIGGER IF EXISTS pages_search_vector ON pages;
DROP FUNCTION IF EXISTS update_pages_search_vector();
DROP INDEX IF EXISTS idx_pages_search;
ALTER TABLE pages DROP COLUMN IF EXISTS search_vector;

-- One normalized TEXT column per weight class (the portable stand-in for
-- `setweight A/B`): title (weight A) and preview (weight B). Existing rows get
-- an empty string; every save recomputes both columns in Rust.
ALTER TABLE pages ADD COLUMN IF NOT EXISTS title_norm TEXT NOT NULL DEFAULT '';
ALTER TABLE pages ADD COLUMN IF NOT EXISTS body_norm  TEXT NOT NULL DEFAULT '';

-- A plain B-tree is enough for a bound `LIKE '%stem%'`; a substring/trigram
-- index can be added later per engine without changing the query.
CREATE INDEX IF NOT EXISTS idx_pages_title_norm ON pages(title_norm);
CREATE INDEX IF NOT EXISTS idx_pages_body_norm  ON pages(body_norm);
