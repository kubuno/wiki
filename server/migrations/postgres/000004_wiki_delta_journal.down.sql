DROP TABLE IF EXISTS change_counter;

-- Restore the pages change-seq sequence + trigger.
CREATE SEQUENCE IF NOT EXISTS page_change_seq;
ALTER TABLE pages ALTER COLUMN change_seq SET DEFAULT nextval('page_change_seq');
CREATE OR REPLACE FUNCTION page_bump_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('page_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER trg_page_change_seq BEFORE UPDATE ON pages
    FOR EACH ROW EXECUTE FUNCTION page_bump_change_seq();

-- Restore the wikis change-seq sequence + trigger.
CREATE SEQUENCE IF NOT EXISTS wiki_change_seq;
ALTER TABLE wikis ALTER COLUMN change_seq SET DEFAULT nextval('wiki_change_seq');
CREATE OR REPLACE FUNCTION wiki_bump_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('wiki_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER trg_wiki_change_seq BEFORE UPDATE ON wikis
    FOR EACH ROW EXECUTE FUNCTION wiki_bump_change_seq();

-- Restore the wiki tombstone trigger.
CREATE OR REPLACE FUNCTION wiki_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO wiki_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('wiki_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
CREATE TRIGGER trg_wiki_tombstone AFTER DELETE ON wikis
    FOR EACH ROW EXECUTE FUNCTION wiki_tombstone();

-- Restore the member → wiki child bump.
CREATE OR REPLACE FUNCTION member_bump_wiki() RETURNS trigger AS $$
BEGIN
    UPDATE wikis SET change_seq = change_seq WHERE id = COALESCE(NEW.wiki_id, OLD.wiki_id);
    RETURN COALESCE(NEW, OLD);
END; $$ LANGUAGE plpgsql;
CREATE TRIGGER trg_member_bump_wiki AFTER INSERT OR UPDATE OR DELETE ON wiki_members
    FOR EACH ROW EXECUTE FUNCTION member_bump_wiki();
