use crate::stores::Listing;
use rusqlite::{Connection, params};
use serde_json::Value;

pub fn open(path: &str) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY, query TEXT NOT NULL, dossier TEXT, verdict TEXT,
            created INTEGER NOT NULL DEFAULT (unixepoch()));
         CREATE TABLE IF NOT EXISTS listings (
            run_id INTEGER NOT NULL REFERENCES runs(id), id INTEGER NOT NULL,
            store TEXT, title TEXT, price_pen REAL, url TEXT, seller TEXT, rating REAL, sold TEXT,
            relevant INTEGER, trust INTEGER, true_cost_pen REAL, flags TEXT, note TEXT, rank INTEGER, why TEXT,
            PRIMARY KEY (run_id, id));",
    )?;
    Ok(db)
}

pub fn new_run(db: &Connection, query: &str, dossier: &Value) -> rusqlite::Result<i64> {
    db.execute("INSERT INTO runs (query, dossier) VALUES (?1, ?2)", params![query, dossier.to_string()])?;
    Ok(db.last_insert_rowid())
}

pub fn save_listings(db: &Connection, run: i64, listings: &[Listing]) -> rusqlite::Result<()> {
    let mut stmt = db.prepare(
        "INSERT INTO listings (run_id, id, store, title, price_pen, url, seller, rating, sold)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for (i, l) in listings.iter().enumerate() {
        stmt.execute(params![run, i as i64, l.store, l.title, l.price_pen, l.url, l.seller, l.rating, l.sold])?;
    }
    Ok(())
}

/// `v` is one verifier verdict: {id, relevant, trust, true_cost_pen, flags, note}.
pub fn save_verdict(db: &Connection, run: i64, v: &Value) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE listings SET relevant = ?3, trust = ?4, true_cost_pen = ?5, flags = ?6, note = ?7
         WHERE run_id = ?1 AND id = ?2",
        params![
            run,
            v["id"].as_i64(),
            v["relevant"].as_bool(),
            v["trust"].as_i64(),
            v["true_cost_pen"].as_f64(),
            v["flags"].to_string(),
            v["note"].as_str()
        ],
    )?;
    Ok(())
}

pub fn save_ranking(db: &Connection, run: i64, ranking: &Value) -> rusqlite::Result<()> {
    for (rank, pick) in ranking["picks"].as_array().into_iter().flatten().enumerate() {
        db.execute(
            "UPDATE listings SET rank = ?3, why = ?4 WHERE run_id = ?1 AND id = ?2",
            params![run, pick["id"].as_i64(), rank as i64 + 1, pick["why"].as_str()],
        )?;
    }
    db.execute("UPDATE runs SET verdict = ?2 WHERE id = ?1", params![run, ranking["verdict"].as_str()])?;
    Ok(())
}

/// Every verified-relevant listing of the run, best ranked first, as CSV.
pub fn export_csv(db: &Connection, run: i64, path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let mut stmt = db.prepare(
        "SELECT rank, store, title, price_pen, true_cost_pen, trust, seller, rating, sold, url, why, note, flags
         FROM listings WHERE run_id = ?1 AND relevant = 1
         ORDER BY rank IS NULL, rank, trust DESC, true_cost_pen",
    )?;
    let mut out = csv::Writer::from_path(path)?;
    out.write_record([
        "rank", "store", "title", "price_pen", "true_cost_pen", "trust", "seller", "rating", "sold", "url", "why",
        "note", "flags",
    ])?;
    let mut rows = stmt.query([run])?;
    let mut n = 0;
    while let Some(r) = rows.next()? {
        let cells: Vec<String> = (0..13)
            .map(|i| match r.get_ref(i) {
                Ok(rusqlite::types::ValueRef::Text(t)) => String::from_utf8_lossy(t).into_owned(),
                Ok(rusqlite::types::ValueRef::Integer(x)) => x.to_string(),
                Ok(rusqlite::types::ValueRef::Real(x)) => format!("{x:.2}"),
                _ => String::new(),
            })
            .collect();
        out.write_record(&cells)?;
        n += 1;
    }
    out.flush()?;
    Ok(n)
}
