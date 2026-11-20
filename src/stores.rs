//! Store experts: one GET per keyword, curated into `Listing`s. Read-only; a blocked store
//! reports the block instead of retrying or evading it.
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub store: String,
    pub title: String,
    pub price_pen: f64,
    pub url: String,
    pub seller: Option<String>,
    pub rating: Option<f64>,
    pub sold: Option<String>,
}

pub const STORES: [&str; 12] = [
    "amazon", "aliexpress", "falabella", "plazavea", "oechsle", "promart", "coolbox", "estilos", "wong", "metro",
    "ripley", "knasta",
];

/// Peruvian stores on VTEX: same public catalog API, so one parser serves all of them.
const VTEX: [(&str, &str); 7] = [
    ("plazavea", "www.plazavea.com.pe"),
    ("oechsle", "www.oechsle.pe"),
    ("promart", "www.promart.pe"),
    ("coolbox", "www.coolbox.pe"),
    ("estilos", "www.estilos.com.pe"),
    ("wong", "www.wong.pe"),
    ("metro", "www.metro.pe"),
];

fn vtex_host(store: &str) -> Option<&'static str> {
    VTEX.iter().find(|(name, _)| *name == store).map(|(_, host)| *host)
}

/// Stores whose search page embeds schema.org JSON-LD `Product`s with prices: one generic parser.
/// `{q}` is the query with spaces as %20 (Ripley takes it in the path).
/// shortcut: templates live in code like VTEX; move them to package manifests when users add their own.
const JSONLD: [(&str, &str, &str); 2] = [
    ("ripley", "simple.ripley.com.pe", "https://simple.ripley.com.pe/search/{q}"),
    ("knasta", "knasta.pe", "https://knasta.pe/results?q={q}"),
];

fn jsonld_store(store: &str) -> Option<(&'static str, &'static str)> {
    JSONLD.iter().find(|(name, _, _)| *name == store).map(|(_, host, tpl)| (*host, *tpl))
}

pub fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36")
        .default_headers(
            [(reqwest::header::ACCEPT_LANGUAGE, "es-PE,es;q=0.9".parse().unwrap())].into_iter().collect(),
        )
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("http client")
}

pub fn search(client: &reqwest::blocking::Client, store: &str, keyword: &str) -> Result<Vec<Listing>, String> {
    let q = encode(keyword);
    let url = match store {
        "amazon" => format!("https://www.amazon.com/s?k={q}"),
        "aliexpress" => format!("https://www.aliexpress.com/w/wholesale-{}.html", q.replace('+', "-")),
        "falabella" => format!("https://www.falabella.com.pe/falabella-pe/search?Ntt={q}"),
        // VTEX rejects "+" for spaces (HTTP 400); it wants %20.
        _ => match (vtex_host(store), jsonld_store(store)) {
            (Some(host), _) => format!("https://{host}/api/catalog_system/pub/products/search?ft={}&_from=0&_to=19", q.replace('+', "%20")),
            (None, Some((_, tpl))) => tpl.replace("{q}", &q.replace('+', "%20")),
            (None, None) => return Err(format!("tienda desconocida: {store}")),
        },
    };
    let body = fetch(client, store, &url)?;
    let listings = match store {
        "amazon" => parse_amazon(&body),
        "aliexpress" => parse_aliexpress(&body),
        "falabella" => parse_falabella(&body),
        _ if jsonld_store(store).is_some() => parse_jsonld(store, &body),
        _ => parse_vtex(store, &body),
    };
    Ok(listings.into_iter().filter(|l| l.price_pen > 0.0).collect())
}

fn fetch(client: &reqwest::blocking::Client, store: &str, url: &str) -> Result<String, String> {
    let resp = client.get(url).send().map_err(|e| format!("{store}: {e}"))?;
    let status = resp.status();
    let body = resp.text().map_err(|e| format!("{store}: {e}"))?;
    // Plaza Vea (VTEX) answers 206 for paged results.
    // 429 and 503 are how these stores throttle automated traffic: a block, not an outage.
    if matches!(status.as_u16(), 429 | 503) {
        return Err(format!("{store}: HTTP {status}, {BLOCKED}"));
    }
    if !status.is_success() {
        return Err(format!("{store}: HTTP {status}"));
    }
    if is_challenge(store, &body) {
        return Err(format!("{store}: {BLOCKED}"));
    }
    Ok(body)
}

/// What the product page itself says, read again at verification time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCheck {
    pub url: String,
    pub ok: bool,
    pub title: Option<String>,
    pub price_pen: Option<f64>,
    pub in_stock: Option<bool>,
    pub seller: Option<String>,
    pub rating: Option<f64>,
    pub reviews: Option<u64>,
    /// "page" = read from the product page; "search_recheck" = the item reappeared in a fresh search.
    pub method: String,
    pub note: String,
}

impl PageCheck {
    fn failed(url: &str, method: &str, note: String) -> Self {
        PageCheck { url: url.into(), ok: false, title: None, price_pen: None, in_stock: None, seller: None,
            rating: None, reviews: None, method: method.into(), note }
    }
}

/// Opens one listing again. `title` is only needed for stores whose product page is rendered by
/// JavaScript (AliExpress): there the check re-runs a search and looks for the same item id.
pub fn open(client: &reqwest::blocking::Client, store: &str, url: &str, title: &str) -> PageCheck {
    let expected = |prefix: &str| url.starts_with(prefix);
    let allowed = match store {
        "amazon" => expected("https://www.amazon.com/"),
        "aliexpress" => expected("https://www.aliexpress.com/item/"),
        "falabella" => expected("https://www.falabella.com.pe/"),
        _ => vtex_host(store)
            .or(jsonld_store(store).map(|(host, _)| host))
            .is_some_and(|host| expected(&format!("https://{host}/"))),
    };
    // Trust boundary: the URL comes from a model; never fetch outside the store's own domain.
    if !allowed {
        return PageCheck::failed(url, "page", format!("URL fuera de {store}"));
    }
    match store {
        // Ripley's JSON-LD product URLs often 404; the item is re-found by searching instead.
        "aliexpress" | "ripley" => recheck_by_search(client, store, url, title),
        _ if vtex_host(store).is_some() => {
            // VTEX product pages render prices client-side; its catalog API answers by slug.
            let host = vtex_host(store).unwrap_or_default();
            let slug = url.trim_end_matches("/p").rsplit('/').next().unwrap_or("");
            let api = format!("https://{host}/api/catalog_system/pub/products/search/{slug}/p");
            match fetch(client, store, &api) {
                Err(e) => PageCheck::failed(url, "page", e),
                Ok(body) => page_vtex(url, &body),
            }
        }
        _ => match fetch(client, store, url) {
            Err(e) => PageCheck::failed(url, "page", e),
            Ok(body) if store == "amazon" => page_amazon(url, &body),
            Ok(body) => page_jsonld(url, &body),
        },
    }
}

fn page_amazon(url: &str, html: &str) -> PageCheck {
    static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"id="productTitle"[^>]*>\s*([^<]+?)\s*<"#).unwrap());
    static PRICE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"a-offscreen">([^<]+)<"#).unwrap());
    static AVAIL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)id="availability".*?<span[^>]*>\s*([^<]+?)\s*<"#).unwrap());
    static SOLD_BY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)offer-display-feature-name="desktop-merchant-info"[^>]*class="offer-display-feature-text[^"]*".*?offer-display-feature-text-message">([^<]+)<|(?s)class="offer-display-feature-text[^"]*"[^>]*offer-display-feature-name="desktop-merchant-info".*?offer-display-feature-text-message">([^<]+)<"#).unwrap()
    });
    static RATING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"id="acrPopover"[^>]*title="([\d.,]+) de 5"#).unwrap());
    static REVIEWS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"acrCustomerReviewText"[^>]*>\(?([\d.,]+)"#).unwrap());
    let Some(title) = TITLE.captures(html).map(|c| unescape(&c[1])) else {
        return PageCheck::failed(url, "page", "la página no tiene título de producto".into());
    };
    let availability = AVAIL.captures(html).map(|c| c[1].to_lowercase());
    // The "sold by" block, not "ships from" or the payment notes beside it.
    let seller = SOLD_BY.captures(html).and_then(|c| c.get(1).or(c.get(2))).map(|m| m.as_str().trim().to_string());
    PageCheck {
        url: url.into(),
        ok: true,
        title: Some(title),
        price_pen: PRICE.captures(html).map(|c| parse_price(&c[1])).filter(|p| *p > 0.0),
        in_stock: availability.as_deref().map(|a| a.contains("disponible") && !a.contains("no disponible")),
        seller,
        rating: RATING.captures(html).map(|c| parse_price(&c[1])),
        reviews: REVIEWS.captures(html).and_then(|c| c[1].replace([',', '.'], "").parse().ok()),
        method: "page".into(),
        note: availability.unwrap_or_else(|| "sin dato de disponibilidad".into()),
    }
}

fn page_jsonld(url: &str, html: &str) -> PageCheck {
    static LD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)<script type="application/ld\+json"[^>]*>(.*?)</script>"#).unwrap());
    static SELLER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""sellerName":"([^"]+)""#).unwrap());
    let product = LD
        .captures_iter(html)
        .filter_map(|c| serde_json::from_str::<Value>(&c[1]).ok())
        .find(|v| v["@type"] == "Product");
    let Some(product) = product else {
        return PageCheck::failed(url, "page", "la página no trae datos de producto".into());
    };
    let offers: Vec<&Value> = match &product["offers"] {
        Value::Array(a) => a.iter().collect(),
        o @ Value::Object(_) => vec![o],
        _ => vec![],
    };
    // Falabella lists the internet price and the normal price; the buyer pays the lowest.
    let best = offers
        .iter()
        .filter_map(|o| o["price"].as_str().map(parse_price).or(o["price"].as_f64()))
        .fold(None::<f64>, |m, p| Some(m.map_or(p, |m| m.min(p))));
    PageCheck {
        url: url.into(),
        ok: true,
        title: product["name"].as_str().map(String::from),
        price_pen: best,
        in_stock: Some(offers.iter().any(|o| o["availability"].as_str().is_some_and(|a| a.ends_with("InStock")))),
        // Falabella names the seller outside the JSON-LD; others put it in offers.seller.
        seller: SELLER
            .captures(html)
            .map(|c| c[1].to_string())
            .or_else(|| offers.iter().find_map(|o| o["seller"]["name"].as_str().map(String::from))),
        rating: product["aggregateRating"]["ratingValue"].as_f64(),
        reviews: product["aggregateRating"]["reviewCount"].as_u64(),
        method: "page".into(),
        note: String::new(),
    }
}

fn page_vtex(url: &str, json: &str) -> PageCheck {
    let Ok(Value::Array(items)) = serde_json::from_str(json) else {
        return PageCheck::failed(url, "page", "respuesta inesperada del catálogo".into());
    };
    let Some(x) = items.first() else {
        return PageCheck::failed(url, "page", "el producto ya no existe".into());
    };
    let seller = &x["items"][0]["sellers"][0];
    let offer = &seller["commertialOffer"];
    PageCheck {
        url: url.into(),
        ok: true,
        title: x["productName"].as_str().map(String::from),
        price_pen: offer["Price"].as_f64(),
        in_stock: Some(offer["AvailableQuantity"].as_f64().unwrap_or(0.0) > 0.0),
        seller: seller["sellerName"].as_str().map(String::from),
        rating: None,
        reviews: None,
        method: "page".into(),
        note: String::new(),
    }
}

/// shortcut: for stores whose product page cannot be read (AliExpress needs JavaScript, Ripley's
/// JSON-LD URLs 404), this only proves the item is still listed at that price in a fresh search;
/// it cannot see stock or the seller. Use a browser-backed MCP to go deeper.
fn recheck_by_search(client: &reqwest::blocking::Client, store: &str, url: &str, title: &str) -> PageCheck {
    let method = "search_recheck";
    // Search ranking shifts between calls; a shorter brand+model query often brings the item back.
    let words: Vec<&str> = title.split_whitespace().collect();
    let queries: Vec<String> = [6, 3].iter().map(|&n| words.iter().take(n).copied().collect::<Vec<_>>().join(" ")).collect();
    let mut found = None;
    for query in queries.iter().filter(|q| !q.is_empty()) {
        match search(client, store, query) {
            Err(e) => return PageCheck::failed(url, method, e),
            Ok(list) => {
                found = list.into_iter().find(|l| l.url == url);
                if found.is_some() {
                    break;
                }
            }
        }
    }
    match found {
        None => PageCheck::failed(url, method, format!("no reapareció al buscar «{}»", queries.join("» ni «"))),
        Some(l) => PageCheck {
            url: url.into(),
            ok: true,
            title: Some(l.title),
            price_pen: Some(l.price_pen),
            in_stock: None,
            seller: None,
            rating: l.rating,
            reviews: None,
            method: method.into(),
            note: l.sold.unwrap_or_default(),
        },
    }
}

/// Marker in every block message, so callers can tell "blocked" from "not found".
pub const BLOCKED: &str = "bloqueado (verificación anti-bot), no se evade";

/// Bot challenges are reported as blocks, never solved: Mercado Libre's account check, Akamai's
/// bm-verify interstitial, Amazon's captcha page and AliExpress's "punish" slider.
fn is_challenge(store: &str, body: &str) -> bool {
    const MARKERS: [&str; 6] =
        ["account-verification", "triggerinterstitialchallenge", "bm-verify=", "validatecaptcha", "/punish", "x5sec"];
    let lower = body.to_ascii_lowercase();
    MARKERS.iter().any(|m| lower.contains(m)) || (store == "amazon" && lower.contains("captcha"))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// "PEN 1,234.56" / "S/ 1,299" / "205,97" → 1234.56 / 1299 / 205.97
pub fn parse_price(s: &str) -> f64 {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
    // A trailing ",dd" is a decimal comma; any other comma groups thousands.
    let normalized = match digits.rfind(',') {
        Some(i) if digits.len() - i == 3 && !digits.contains('.') => digits.replace(',', "."),
        _ => digits.replace(',', ""),
    };
    normalized.parse().unwrap_or(0.0)
}

fn parse_amazon(html: &str) -> Vec<Listing> {
    static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<h2[^>]*aria-label="([^"]+)""#).unwrap());
    static HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"href="(/[^"]*?/dp/[A-Z0-9]{10})"#).unwrap());
    static PRICE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"a-offscreen">([^<]+)<"#).unwrap());
    static RATING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"a-icon-alt">([\d.,]+) de 5"#).unwrap());
    html.split(r#"data-component-type="s-search-result""#)
        .skip(1)
        .filter_map(|card| {
            Some(Listing {
                store: "amazon".into(),
                title: unescape(&TITLE.captures(card)?[1]),
                price_pen: parse_price(&PRICE.captures(card)?[1]),
                url: format!("https://www.amazon.com{}", &HREF.captures(card)?[1]),
                seller: None,
                rating: RATING.captures(card).map(|c| parse_price(&c[1])),
                sold: None,
            })
        })
        .collect()
}

fn parse_aliexpress(html: &str) -> Vec<Listing> {
    let key = r#""itemList":{"content":"#;
    let Some(i) = html.find(key) else { return vec![] };
    let mut items = serde_json::Deserializer::from_str(&html[i + key.len()..]).into_iter::<Vec<Value>>();
    let Some(Ok(items)) = items.next() else { return vec![] };
    items
        .iter()
        .filter_map(|x| {
            let prices = &x["prices"];
            let p = prices["salePrice"]["minPrice"].as_f64().or(prices["originalPrice"]["minPrice"].as_f64())?;
            Some(Listing {
                store: "aliexpress".into(),
                title: x["title"]["displayTitle"].as_str()?.to_string(),
                price_pen: p,
                url: format!("https://www.aliexpress.com/item/{}.html", x["productId"].as_str()?),
                seller: None,
                rating: x["evaluation"]["starRating"].as_f64(),
                sold: x["trade"]["tradeDesc"].as_str().map(String::from),
            })
        })
        .collect()
}

fn parse_falabella(html: &str) -> Vec<Listing> {
    static NEXT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap());
    let Some(data) = NEXT.captures(html).and_then(|c| serde_json::from_str::<Value>(&c[1]).ok()) else {
        return vec![];
    };
    let Some(results) = data["props"]["pageProps"]["results"].as_array() else { return vec![] };
    results
        .iter()
        .filter_map(|x| {
            Some(Listing {
                store: "falabella".into(),
                title: format!("{} {}", x["brand"].as_str().unwrap_or(""), x["displayName"].as_str()?),
                price_pen: parse_price(x["prices"][0]["price"][0].as_str()?),
                url: x["url"].as_str()?.to_string(),
                seller: x["sellerName"].as_str().map(String::from),
                rating: None,
                sold: None,
            })
        })
        .collect()
}

/// Every schema.org `Product` in the page's JSON-LD, found through `@graph`, `mainEntity`,
/// `itemListElement` and `item`. Only priced offers in PEN (or unlabeled) and not out of stock count.
fn parse_jsonld(store: &str, html: &str) -> Vec<Listing> {
    static LD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)<script[^>]*application/ld\+json[^>]*>(.*?)</script>"#).unwrap());
    fn products<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| products(x, out)),
            Value::Object(o) if o.get("@type").is_some_and(|t| t == "Product") => out.push(v),
            Value::Object(o) => ["@graph", "mainEntity", "itemListElement", "item"]
                .iter()
                .filter_map(|k| o.get(*k))
                .for_each(|x| products(x, out)),
            _ => {}
        }
    }
    let docs: Vec<Value> = LD.captures_iter(html).filter_map(|c| serde_json::from_str(&c[1]).ok()).collect();
    let mut found = Vec::new();
    docs.iter().for_each(|d| products(d, &mut found));
    let num = |v: &Value| v.as_f64().or_else(|| v.as_str().map(parse_price)).filter(|p| *p > 0.0);
    found
        .into_iter()
        .filter_map(|p| {
            let offer = match &p["offers"] {
                Value::Array(a) => a.first()?,
                o => o,
            };
            let currency = offer["priceCurrency"].as_str().unwrap_or("PEN");
            let out_of_stock = offer["availability"].as_str().is_some_and(|a| a.ends_with("OutOfStock"));
            if currency != "PEN" || out_of_stock {
                return None;
            }
            Some(Listing {
                store: store.into(),
                title: p["name"].as_str()?.trim().to_string(),
                // AggregateOffer carries lowPrice instead of price.
                price_pen: num(&offer["price"]).or_else(|| num(&offer["lowPrice"]))?,
                url: p["url"].as_str().or(offer["url"].as_str()).filter(|u| u.starts_with("https://"))?.to_string(),
                seller: offer["seller"]["name"].as_str().map(String::from),
                rating: num(&p["aggregateRating"]["ratingValue"]),
                sold: None,
            })
        })
        .collect()
}

fn parse_vtex(store: &str, json: &str) -> Vec<Listing> {
    let Ok(Value::Array(items)) = serde_json::from_str(json) else { return vec![] };
    items
        .iter()
        .filter_map(|x| {
            let seller = &x["items"][0]["sellers"][0];
            let offer = &seller["commertialOffer"];
            if offer["AvailableQuantity"].as_f64().unwrap_or(0.0) <= 0.0 {
                return None;
            }
            Some(Listing {
                store: store.into(),
                title: x["productName"].as_str()?.to_string(),
                price_pen: offer["Price"].as_f64()?,
                url: x["link"].as_str()?.to_string(),
                seller: seller["sellerName"].as_str().map(String::from),
                rating: None,
                sold: None,
            })
        })
        .collect()
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_price_formats() {
        assert_eq!(parse_price("PEN 1,234.56"), 1234.56);
        assert_eq!(parse_price("S/ 1,299"), 1299.0);
        assert_eq!(parse_price("PEN 205,97"), 205.97);
        assert_eq!(parse_price("310"), 310.0);
    }

    #[test]
    fn parses_amazon_card() {
        let html = r#"x<div data-component-type="s-search-result"><a href="/-/es/FiiO-JT3/dp/B0ABCDEFGH/ref=sr_1"></a>
            <h2 aria-label="FiiO JT3 &amp; cable"><span class="a-offscreen">PEN 204.62</span>
            <span class="a-icon-alt">4.5 de 5 estrellas</span></div>"#;
        let l = &parse_amazon(html)[0];
        assert_eq!((l.title.as_str(), l.price_pen, l.rating), ("FiiO JT3 & cable", 204.62, Some(4.5)));
        assert_eq!(l.url, "https://www.amazon.com/-/es/FiiO-JT3/dp/B0ABCDEFGH");
    }

    #[test]
    fn reads_product_pages() {
        let amz = r#"<span id="productTitle" class="a">  FiiO JT3  </span><span class="a-offscreen">PEN 204.62</span>
            <div id="availability" class="x"> <span class="a"> Disponible </span></div>
            <div offer-display-feature-name="desktop-fulfiller-info"><span class="offer-display-feature-text-message">Amazon</span></div>
            <div class="offer-display-feature-text a-size-small" offer-display-feature-name="desktop-merchant-info"><span class="offer-display-feature-text-message">FiiO Store</span></div>
            <div offer-display-feature-name="desktop-dynamic-secure-transaction"><span class="offer-display-feature-text-message">Transacción segura</span></div>
            <span id="acrPopover" title="4.6 de 5 estrellas"></span><span id="acrCustomerReviewText" aria-label="x">(1,234)</span>"#;
        let c = page_amazon("u", amz);
        assert_eq!((c.ok, c.price_pen, c.in_stock, c.seller.as_deref()), (true, Some(204.62), Some(true), Some("FiiO Store")));
        assert_eq!((c.rating, c.reviews), (Some(4.6), Some(1234)));

        let fal = r#"<script type="application/ld+json">{"@type":"Product","name":"ATH M40X","offers":[
            {"price":"510","availability":"https://schema.org/InStock"},{"price":"619","availability":"https://schema.org/InStock"}]}</script>
            "sellerName":"Live Music""#;
        let c = page_jsonld("u", fal);
        assert_eq!((c.price_pen, c.in_stock, c.seller.as_deref()), (Some(510.0), Some(true), Some("Live Music")));

        let pv = r#"[{"productName":"Space One","items":[{"sellers":[{"sellerName":"SanyPeru","commertialOffer":{"Price":399.0,"AvailableQuantity":0}}]}]}]"#;
        let c = page_vtex("u", pv);
        assert_eq!((c.price_pen, c.in_stock), (Some(399.0), Some(false)));
        assert!(!page_vtex("u", "[]").ok);
    }

    #[test]
    fn challenge_pages_are_blocks() {
        assert!(is_challenge("amazon", "<script>function triggerInterstitialChallenge() {}</script>"));
        assert!(is_challenge("falabella", "https://x/account-verification?go"));
        assert!(is_challenge("amazon", r#"<form action="/errors_page/validateCaptcha">"#));
        assert!(is_challenge("aliexpress", r#"<script src="/_____tmd_____/punish?x5secdata=1">"#));
        assert!(!is_challenge("falabella", "<h2 aria-label=\"FiiO JT3\">"));
    }

    #[test]
    fn vtex_stores_share_one_parser() {
        let json = r#"[{"productName":"KZ EDX Pro","link":"https://www.oechsle.pe/kz-edx-pro/p",
            "items":[{"sellers":[{"sellerName":"oechsle","commertialOffer":{"Price":49.9,"AvailableQuantity":3}}]}]}]"#;
        let l = &parse_vtex("oechsle", json)[0];
        assert_eq!((l.store.as_str(), l.price_pen, l.seller.as_deref()), ("oechsle", 49.9, Some("oechsle")));
        assert!(STORES.iter().all(|s| ["amazon", "aliexpress", "falabella"].contains(s) || vtex_host(s).is_some() || jsonld_store(s).is_some()));
        // A VTEX store may not open URLs of another VTEX store.
        let c = open(&client(), "oechsle", "https://www.plazavea.com.pe/x/p", "");
        assert!(!c.ok && c.note.contains("fuera"));
    }

    #[test]
    fn reads_jsonld_products() {
        let html = r#"<script type="application/ld+json">{"@type":"ItemList","itemListElement":[
            {"@type":"ListItem","item":{"@type":"Product","name":"AUDÍFONOS AIWA AW-K56","url":"https://simple.ripley.com.pe/a/1",
              "offers":{"@type":"Offer","priceCurrency":"PEN","price":"69","availability":"https://schema.org/InStock"}}},
            {"@type":"ListItem","item":{"@type":"Product","name":"Agotado","url":"https://simple.ripley.com.pe/a/2",
              "offers":{"priceCurrency":"PEN","price":"10","availability":"https://schema.org/OutOfStock"}}},
            {"@type":"ListItem","item":{"@type":"Product","name":"En dólares","url":"https://x/3","offers":{"priceCurrency":"USD","price":"5"}}}]}</script>
            <script type="application/ld+json">{"@graph":[{"@type":"Product","name":"Sony WH-1000XM5",
              "offers":{"url":"https://knasta.pe/detail/shopstar/1/x","lowPrice":729,"seller":{"name":"Shopstar"}},
              "aggregateRating":{"ratingValue":"4.7"}}]}</script>"#;
        let l = parse_jsonld("ripley", html);
        assert_eq!(l.len(), 2, "out-of-stock and non-PEN offers are dropped");
        assert_eq!((l[0].title.as_str(), l[0].price_pen), ("AUDÍFONOS AIWA AW-K56", 69.0));
        assert_eq!((l[1].price_pen, l[1].seller.as_deref(), l[1].rating), (729.0, Some("Shopstar"), Some(4.7)));
        assert_eq!(l[1].url, "https://knasta.pe/detail/shopstar/1/x");
        let c = open(&client(), "knasta", "https://simple.ripley.com.pe/a/1", "");
        assert!(!c.ok && c.note.contains("fuera"), "each JSON-LD store only opens its own host");
    }

    #[test]
    fn open_refuses_other_domains() {
        let c = open(&client(), "amazon", "https://evil.example.com/dp/B000000000", "");
        assert!(!c.ok && c.note.contains("fuera"));
    }

    #[test]
    fn parses_aliexpress_item_list() {
        let html = r#"..."itemList":{"content":[{"productId":"100","title":{"displayTitle":"FiiO JT3"},
            "prices":{"salePrice":{"minPrice":202.08}},"evaluation":{"starRating":4.9},"trade":{"tradeDesc":"46 vendidos"}}],"x":1}"#;
        let l = &parse_aliexpress(html)[0];
        assert_eq!((l.price_pen, l.sold.as_deref()), (202.08, Some("46 vendidos")));
    }
}
