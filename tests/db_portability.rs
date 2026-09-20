//! Runs the wiki module's own migrations and delta/search primitives against a
//! real server of **each** engine, from a single compiled binary — the proof
//! that the engine is a run-time choice, not a build-time one, and that the two
//! recently-ported primitives (the change journal and the normalized full-text
//! search) behave identically on all of them.
//!
//! The page CRUD path proper is drive-backed (the `.kbwik` files live in the
//! `drive` module, reached over HTTP), so it cannot run in a unit test. This
//! binary exercises the DATABASE layer directly, issuing the very SQL the
//! services issue (id + change_seq generated in Rust, `title_norm`/`body_norm`
//! from `search::normalize`), which is what the port changed.
//!
//! * SQLite always runs (a temp file, no server).
//! * PostgreSQL runs when `KUBUNO_PG_TEST_URL` points at a throwaway database.
//! * MySQL/MariaDB runs when `KUBUNO_MYSQL_TEST_URL` does.
//!
//! ```sh
//! KUBUNO_PG_TEST_URL=postgres://u:p@127.0.0.1:5433/wiki \
//! KUBUNO_MYSQL_TEST_URL=mysql://u:p@127.0.0.1:3307/wiki \
//!   SQLX_OFFLINE=true cargo test --test db_portability
//! ```

use kubuno_db::search::{self, Field, Query, Weight};
use kubuno_db::{new_id, params};
use kubuno_wiki::{sync, SCHEMA};
use uuid::Uuid;

fn base_settings(engine: &str) -> kubuno_db::DbSettings {
    kubuno_db::DbSettings {
        engine: engine.to_string(),
        url: None,
        host: None,
        port: None,
        user: None,
        password: None,
        database: None,
        path: None,
        max_connections: 4,
        min_connections: 0,
        connect_timeout: std::time::Duration::from_secs(10),
        run_migrations: true,
    }
}

/// Migrations run one at a time: the PostgreSQL and MySQL suites may share a server.
static EXCLUSIVE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn migrated_pool(settings: kubuno_db::DbSettings) -> (kubuno_db::DbPool, impl Sized) {
    let guard = EXCLUSIVE.lock().await;
    let pool = kubuno_db::connect(&settings, SCHEMA).await.expect("connect");
    kubuno_db::migrations!(
        "./migrations/postgres",
        "./migrations/mysql",
        "./migrations/sqlite",
    )
    .run(&pool, SCHEMA)
    .await
    .expect("migrations");
    (pool, guard)
}

// ── Direct DB writes mirroring the services (no drive dependency) ────────────

async fn insert_wiki(pool: &kubuno_db::DbPool, owner: Uuid, name: &str, slug: &str) -> Uuid {
    let id = new_id();
    let mut tx = pool.begin().await.expect("begin");
    let seq = sync::next_wiki_seq(&mut tx).await.expect("wiki seq");
    tx.execute(
        "INSERT INTO wiki.wikis (id, owner_id, storage_owner_id, slug, name, description, is_shared, change_seq) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        params![id, owner, owner, slug, name, "", false, seq],
    )
    .await
    .expect("insert wiki");
    tx.commit().await.expect("commit");
    id
}

async fn insert_page(pool: &kubuno_db::DbPool, wiki_id: Uuid, title: &str, slug: &str, preview: &str) -> Uuid {
    let id = new_id();
    let title_norm = search::normalize(title);
    let body_norm = search::normalize(preview);
    let mut tx = pool.begin().await.expect("begin");
    let seq = sync::next_page_seq(&mut tx).await.expect("page seq");
    tx.execute(
        "INSERT INTO wiki.pages (id, wiki_id, namespace, title, slug, file_id, redirect_to, preview, \
            byte_size, current_author_id, current_rev_at, title_norm, body_norm, change_seq) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        params![
            id, wiki_id, "Main", title, slug, new_id(), None::<&str>, preview,
            preview.len() as i32, None::<Uuid>, chrono::Utc::now(), &title_norm, &body_norm, seq
        ],
    )
    .await
    .expect("insert page");
    tx.commit().await.expect("commit");
    id
}

/// Re-edit a page (bumps its change_seq, as `save_page` does on an update).
async fn touch_page(pool: &kubuno_db::DbPool, id: Uuid, new_preview: &str) {
    let body_norm = search::normalize(new_preview);
    let mut tx = pool.begin().await.expect("begin");
    let seq = sync::next_page_seq(&mut tx).await.expect("page seq");
    tx.execute(
        "UPDATE wiki.pages SET preview = $1, body_norm = $2, change_seq = $3 WHERE id = $4",
        params![new_preview, &body_norm, seq, id],
    )
    .await
    .expect("update page");
    tx.commit().await.expect("commit");
}

/// Soft-delete a page (a `modified` change carrying is_deleted=true).
async fn soft_delete_page(pool: &kubuno_db::DbPool, id: Uuid) {
    let mut tx = pool.begin().await.expect("begin");
    let seq = sync::next_page_seq(&mut tx).await.expect("page seq");
    tx.execute(
        "UPDATE wiki.pages SET is_deleted = $1, change_seq = $2 WHERE id = $3",
        params![true, seq, id],
    )
    .await
    .expect("soft delete");
    tx.commit().await.expect("commit");
}

async fn delete_wiki(pool: &kubuno_db::DbPool, wiki_id: Uuid, owner: Uuid) {
    let mut tx = pool.begin().await.expect("begin");
    let seq = sync::next_wiki_seq(&mut tx).await.expect("wiki seq");
    sync::record_wiki_tombstone(&mut tx, wiki_id, owner, seq).await.expect("tombstone");
    tx.execute("DELETE FROM wiki.wikis WHERE id = $1", params![wiki_id]).await.expect("delete");
    tx.commit().await.expect("commit");
}

async fn page_seq(pool: &kubuno_db::DbPool, id: Uuid) -> i64 {
    pool.fetch_scalar::<i64>("SELECT change_seq FROM wiki.pages WHERE id = $1", params![id])
        .await
        .expect("page seq")
}

/// The owner-scoped page feed the `/pages/delta` handler reads (pages carry no
/// tombstone; a soft-delete rides as `is_deleted`).
#[derive(Debug, sqlx::FromRow)]
struct PageFeedRow {
    id:         Uuid,
    change_seq: i64,
    is_deleted: bool,
}

async fn pages_since(pool: &kubuno_db::DbPool, owner: Uuid, cursor: i64) -> Vec<PageFeedRow> {
    pool.fetch_all_as::<PageFeedRow>(
        "SELECT p.id, p.change_seq, p.is_deleted \
         FROM wiki.pages p JOIN wiki.wikis w ON w.id = p.wiki_id \
         WHERE w.owner_id = $1 AND p.change_seq > $2 ORDER BY p.change_seq",
        params![owner, cursor],
    )
    .await
    .expect("pages feed")
}

#[derive(Debug, sqlx::FromRow)]
struct TitleRow {
    title: String,
}

/// The exact search the module runs: `Query::build` over the two normalized,
/// weighted columns, best-ranked first.
async fn search_titles(pool: &kubuno_db::DbPool, wiki_id: Uuid, query: &str) -> Vec<String> {
    let fields = [
        Field::new("title_norm", Weight::A),
        Field::new("body_norm", Weight::B),
    ];
    let Some(s) = Query::build(query, &fields, 2) else {
        return Vec::new();
    };
    let sql = format!(
        "SELECT title FROM wiki.pages \
         WHERE wiki_id = $1 AND NOT is_deleted AND {} \
         ORDER BY {} DESC, title ASC LIMIT ${}",
        s.where_sql, s.order_sql, s.next
    );
    let mut binds = params![wiki_id];
    binds.extend(s.binds);
    binds.push(100i64.into());
    pool.fetch_all_as::<TitleRow>(&sql, binds)
        .await
        .expect("search")
        .into_iter()
        .map(|t| t.title)
        .collect()
}

async fn full_suite(pool: &kubuno_db::DbPool) {
    let owner = Uuid::new_v4();
    let wiki = insert_wiki(pool, owner, "Manuel", "manuel").await;

    // The wiki's creation gave it a change_seq > 0, visible through the journal
    // primitive `changes_since` (wikis carry an owner_id, so this works directly).
    let wiki_changes = kubuno_db::journal::changes_since(
        pool, sync::WIKIS_TABLE, sync::WIKI_TOMBSTONES, owner, 0, 10_000,
    )
    .await
    .expect("wiki delta");
    let wiki_seq_0 = wiki_changes.iter().map(|c| c.change_seq).max().unwrap_or(0);
    assert!(wiki_seq_0 > 0, "the wiki carries a change_seq after creation");

    // ── pages: strict change_seq monotonicity across create / edit / delete ──
    let mut seqs: Vec<i64> = Vec::new();

    let p1 = insert_page(pool, wiki, "Un cheval au galop", "un_cheval_au_galop",
                         "Le cheval broute dans le pré développé").await;
    seqs.push(page_seq(pool, p1).await);

    let p2 = insert_page(pool, wiki, "Recette de café", "recette_de_cafe",
                         "Boire un café le matin").await;
    seqs.push(page_seq(pool, p2).await);

    let p3 = insert_page(pool, wiki, "Notes de résumé", "notes_de_resume",
                         "Le résumé du projet est prêt").await;
    seqs.push(page_seq(pool, p3).await);

    // edit p1 → its change_seq must advance past p2 and p3.
    touch_page(pool, p1, "Le cheval broute encore dans le pré").await;
    seqs.push(page_seq(pool, p1).await);

    // soft-delete p2 → a fresh (greater) change_seq, still a live row.
    soft_delete_page(pool, p2).await;
    seqs.push(page_seq(pool, p2).await);

    for w in seqs.windows(2) {
        assert!(w[1] > w[0], "change_seq must strictly increase: {seqs:?}");
    }

    // The owner-scoped page feed shows p2 as is_deleted=true (a modified change,
    // no tombstone) and p1 as a live modified row.
    let feed = pages_since(pool, owner, 0).await;
    assert!(feed.iter().any(|r| r.id == p2 && r.is_deleted), "p2 rides as is_deleted");
    assert!(feed.iter().any(|r| r.id == p1 && !r.is_deleted), "p1 is a live modified row");
    // The feed is ordered by change_seq.
    let feed_seqs: Vec<i64> = feed.iter().map(|r| r.change_seq).collect();
    let mut sorted = feed_seqs.clone();
    sorted.sort_unstable();
    assert_eq!(feed_seqs, sorted, "the page feed is ordered by change_seq");

    // ── search: stemmed + deaccented, identical on every engine ──
    // 1. A plural query word finds the singular stored form: "chevaux" stems to
    //    "cheval", which p1 contains (a soft-deleted p2 is excluded).
    let hits = search_titles(pool, wiki, "chevaux").await;
    assert_eq!(hits, vec!["Un cheval au galop".to_owned()], "chevaux -> cheval");

    // 2. Accent folding: "resume" (no accents) finds "résumé".
    let hits = search_titles(pool, wiki, "resume").await;
    assert_eq!(hits, vec!["Notes de résumé".to_owned()], "resume -> résumé");

    // 3. A term absent everywhere returns nothing.
    assert!(search_titles(pool, wiki, "hélicoptère").await.is_empty());

    // ── ranking: a title hit (weight A) outranks a body-only hit (weight B) ──
    let rank_wiki = insert_wiki(pool, owner, "Zoo", "zoo").await;
    // "lion" in the TITLE only.
    insert_page(pool, rank_wiki, "Le lion majestueux", "le_lion", "un grand félin").await;
    // "lion" in the PREVIEW only.
    insert_page(pool, rank_wiki, "Félins divers", "felins_divers", "le lion et le tigre").await;
    let ranked = search_titles(pool, rank_wiki, "lion").await;
    assert_eq!(
        ranked,
        vec!["Le lion majestueux".to_owned(), "Félins divers".to_owned()],
        "the title hit ranks above the body-only hit"
    );

    // ── tombstone: deleting a wiki surfaces as a tombstone in changes_since ──
    let doomed = insert_wiki(pool, owner, "Éphémère", "ephemere").await;
    delete_wiki(pool, doomed, owner).await;
    let wiki_changes = kubuno_db::journal::changes_since(
        pool, sync::WIKIS_TABLE, sync::WIKI_TOMBSTONES, owner, 0, 10_000,
    )
    .await
    .expect("wiki delta");
    assert!(
        wiki_changes.iter().any(|c| c.id == doomed && c.deleted),
        "the deleted wiki must appear as a tombstone"
    );
    assert!(
        wiki_changes.iter().any(|c| c.id == wiki && !c.deleted),
        "the live wiki must appear as a modified row"
    );
    // Wiki change_seqs are strictly monotonic too (create < ... < tombstone).
    let mut wiki_seqs: Vec<i64> = wiki_changes.iter().map(|c| c.change_seq).collect();
    let uniq = {
        let mut s = wiki_seqs.clone();
        s.sort_unstable();
        s.dedup();
        s.len()
    };
    wiki_seqs.sort_unstable();
    assert_eq!(uniq, wiki_seqs.len(), "wiki change_seqs are unique (monotonic counter)");
}

#[tokio::test]
async fn sqlite_from_the_one_binary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut s = base_settings("sqlite");
    s.path = Some(dir.path().to_string_lossy().into_owned());
    let (pool, _keep) = migrated_pool(s).await;
    full_suite(&pool).await;
}

#[tokio::test]
async fn postgres_from_the_one_binary() {
    let Ok(url) = std::env::var("KUBUNO_PG_TEST_URL") else {
        eprintln!("skipping: KUBUNO_PG_TEST_URL not set");
        return;
    };
    let mut s = base_settings("postgres");
    s.url = Some(url);
    let (pool, _keep) = migrated_pool(s).await;
    full_suite(&pool).await;
}

#[tokio::test]
async fn mysql_from_the_one_binary() {
    let Ok(url) = std::env::var("KUBUNO_MYSQL_TEST_URL") else {
        eprintln!("skipping: KUBUNO_MYSQL_TEST_URL not set");
        return;
    };
    let mut s = base_settings("mysql");
    s.url = Some(url);
    let (pool, _keep) = migrated_pool(s).await;
    full_suite(&pool).await;
}
