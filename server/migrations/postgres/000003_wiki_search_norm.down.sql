DROP INDEX IF EXISTS idx_pages_body_norm;
DROP INDEX IF EXISTS idx_pages_title_norm;
ALTER TABLE pages DROP COLUMN IF EXISTS body_norm;
ALTER TABLE pages DROP COLUMN IF EXISTS title_norm;

ALTER TABLE pages ADD COLUMN IF NOT EXISTS search_vector TSVECTOR;
CREATE INDEX IF NOT EXISTS idx_pages_search ON pages USING GIN(search_vector);

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
