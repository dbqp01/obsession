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
            PRIMARY KEY (run_id, id));
         CREATE TABLE IF NOT EXISTS negotiations (
            id INTEGER PRIMARY KEY, run_id INTEGER REFERENCES runs(id), listing_url TEXT NOT NULL,
            mode TEXT NOT NULL, status TEXT NOT NULL, case_json TEXT NOT NULL, our_offer REAL, seller_offer REAL,
            concessions INTEGER DEFAULT 0, trust INTEGER, signals TEXT, created INTEGER DEFAULT (unixepoch()),
            updated INTEGER);
         CREATE TABLE IF NOT EXISTS neg_messages (
            negotiation_id INTEGER REFERENCES negotiations(id), ts INTEGER NOT NULL, role TEXT NOT NULL,
            text TEXT, meta TEXT);",
    )?;
    // Older databases predate the report column; adding it twice is the only expected error.
    let _ = db.execute("ALTER TABLE runs ADD COLUMN report TEXT", []);
    Ok(db)
}

pub fn new_run(db: &Connection, query: &str, dossier: &Value) -> rusqlite::Result<i64> {
    db.execute("INSERT INTO runs (query, dossier) VALUES (?1, ?2)", params![query, dossier.to_string()])?;
    Ok(db.last_insert_rowid())
}

pub fn save_report(db: &Connection, run: i64, report: &Value) -> rusqlite::Result<()> {
    db.execute("UPDATE runs SET report = ?2 WHERE id = ?1", params![run, report.to_string()])?;
    Ok(())
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

/// One negotiation row; `case_json` and `signals` stay JSON so the turn reads them as the model sees them.
#[derive(Debug, Clone)]
pub struct Negotiation {
    pub id: i64,
    pub run_id: Option<i64>,
    pub status: String,
    pub case: Value,
    pub our_offer: Option<f64>,
    pub seller_offer: Option<f64>,
    pub concessions: i64,
    pub signals: Vec<String>,
    pub created: i64,
}

#[derive(Debug, Clone)]
pub struct NegMessage {
    pub ts: i64,
    pub role: String,
    pub text: String,
    pub meta: Value,
}

pub fn new_negotiation(db: &Connection, run_id: Option<i64>, case: &Value, status: &str, now: i64) -> rusqlite::Result<i64> {
    db.execute(
        "INSERT INTO negotiations (run_id, listing_url, mode, status, case_json, trust, signals, created, updated)
         VALUES (?1, ?2, 'copilot', ?3, ?4, ?5, '[]', ?6, ?6)",
        params![run_id, case["listing"]["url"].as_str().unwrap_or(""), status, case.to_string(),
            case["trust"]["score"].as_i64(), now],
    )?;
    Ok(db.last_insert_rowid())
}

pub fn negotiation(db: &Connection, id: i64) -> rusqlite::Result<Negotiation> {
    db.query_row(
        "SELECT id, run_id, status, case_json, our_offer, seller_offer, concessions, signals, created
         FROM negotiations WHERE id = ?1",
        [id],
        |r| {
            let json = |i: usize| r.get::<_, Option<String>>(i).map(|t| t.and_then(|t| serde_json::from_str(&t).ok()));
            Ok(Negotiation {
                id: r.get(0)?,
                run_id: r.get(1)?,
                status: r.get(2)?,
                case: json(3)?.unwrap_or_default(),
                our_offer: r.get(4)?,
                seller_offer: r.get(5)?,
                concessions: r.get(6)?,
                signals: serde_json::from_value(json(7)?.unwrap_or_default()).unwrap_or_default(),
                created: r.get(8)?,
            })
        },
    )
}

pub fn negotiation_ids(db: &Connection) -> rusqlite::Result<Vec<i64>> {
    db.prepare("SELECT id FROM negotiations ORDER BY id")?.query_map([], |r| r.get(0))?.collect()
}

pub fn save_negotiation(db: &Connection, n: &Negotiation, now: i64) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE negotiations SET status = ?2, our_offer = ?3, seller_offer = ?4, concessions = ?5, signals = ?6,
         updated = ?7 WHERE id = ?1",
        params![n.id, n.status, n.our_offer, n.seller_offer, n.concessions, serde_json::to_string(&n.signals).unwrap(), now],
    )?;
    Ok(())
}

pub fn add_neg_message(db: &Connection, id: i64, ts: i64, role: &str, text: &str, meta: &Value) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO neg_messages (negotiation_id, ts, role, text, meta) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, ts, role, text, meta.to_string()],
    )?;
    Ok(())
}

/// Messages in insertion order (rowid), so same-second rows keep the order they happened in.
pub fn neg_messages(db: &Connection, id: i64) -> rusqlite::Result<Vec<NegMessage>> {
    db.prepare("SELECT ts, role, text, meta FROM neg_messages WHERE negotiation_id = ?1 ORDER BY rowid")?
        .query_map([id], |r| {
            Ok(NegMessage {
                ts: r.get(0)?,
                role: r.get(1)?,
                text: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                meta: r.get::<_, Option<String>>(3)?.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default(),
            })
        })?
        .collect()
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
