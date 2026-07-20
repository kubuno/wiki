-- Delta primitives for the local-first pull (wikis, pages). Pages are SOFT-deleted
-- (is_deleted flag) so they never need a tombstone — a soft-delete is just a
-- `modified` change carrying is_deleted=true. Only wiki hard-deletes tombstone
-- (the client then drops the wiki and its pages). wiki_members bump their wiki.

-- ===== wikis =====
CREATE SEQUENCE IF NOT EXISTS wiki_change_seq;
ALTER TABLE wikis ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('wiki_change_seq');
CREATE INDEX IF NOT EXISTS idx_wikis_change_seq ON wikis(owner_id, change_seq);
CREATE OR REPLACE FUNCTION wiki_bump_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('wiki_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_wiki_change_seq ON wikis;
CREATE TRIGGER trg_wiki_change_seq BEFORE UPDATE ON wikis
    FOR EACH ROW EXECUTE FUNCTION wiki_bump_change_seq();
CREATE TABLE IF NOT EXISTS wiki_tombstones (
    id UUID PRIMARY KEY, owner_id UUID NOT NULL, change_seq BIGINT NOT NULL, deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
CREATE INDEX IF NOT EXISTS idx_wiki_tomb ON wiki_tombstones(owner_id, change_seq);
CREATE OR REPLACE FUNCTION wiki_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO wiki_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('wiki_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_wiki_tombstone ON wikis;
CREATE TRIGGER trg_wiki_tombstone AFTER DELETE ON wikis
    FOR EACH ROW EXECUTE FUNCTION wiki_tombstone();

-- wiki_members bump their wiki (members inline in the wiki delta)
CREATE OR REPLACE FUNCTION member_bump_wiki() RETURNS trigger AS $$
BEGIN
    UPDATE wikis SET change_seq = change_seq WHERE id = COALESCE(NEW.wiki_id, OLD.wiki_id);
    RETURN COALESCE(NEW, OLD);
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_member_bump_wiki ON wiki_members;
CREATE TRIGGER trg_member_bump_wiki AFTER INSERT OR UPDATE OR DELETE ON wiki_members
    FOR EACH ROW EXECUTE FUNCTION member_bump_wiki();

-- ===== pages (no tombstone — soft-deleted) =====
CREATE SEQUENCE IF NOT EXISTS page_change_seq;
ALTER TABLE pages ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('page_change_seq');
CREATE INDEX IF NOT EXISTS idx_pages_change_seq ON pages(wiki_id, change_seq);
CREATE OR REPLACE FUNCTION page_bump_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('page_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_page_change_seq ON pages;
CREATE TRIGGER trg_page_change_seq BEFORE UPDATE ON pages
    FOR EACH ROW EXECUTE FUNCTION page_bump_change_seq();
