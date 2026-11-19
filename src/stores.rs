//! Store experts: one GET per keyword, curated into `Listing`s. Read-only; a blocked store
//! reports the block instead of retrying or evading it.
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    pub store: &'static str,
    pub title: String,
    pub price_pen: f64,
    pub url: String,
    pub seller: Option<String>,
    pub rating: Option<f64>,
    pub sold: Option<String>,
}

pub const STORES: [&str; 4] = ["amazon", "aliexpress", "falabella", "plazavea"];

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
        "plazavea" => format!("https://www.plazavea.com.pe/api/catalog_system/pub/products/search?ft={}&_from=0&_to=19", q.replace('+', "%20")),
        _ => return Err(format!("tienda desconocida: {store}")),
    };
    let resp = client.get(&url).send().map_err(|e| format!("{store}: {e}"))?;
    let status = resp.status();
    let body = resp.text().map_err(|e| format!("{store}: {e}"))?;
    // Plaza Vea (VTEX) answers 206 for paged results.
    if !status.is_success() {
        return Err(format!("{store}: HTTP {status}"));
    }
    if body.contains("account-verification") || (store == "amazon" && body.contains("captcha")) {
        return Err(format!("{store}: bloqueado (verificación anti-bot), no se evade"));
    }
    let listings = match store {
        "amazon" => parse_amazon(&body),
        "aliexpress" => parse_aliexpress(&body),
        "falabella" => parse_falabella(&body),
        _ => parse_plazavea(&body),
    };
    Ok(listings.into_iter().filter(|l| l.price_pen > 0.0).collect())
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
fn price(s: &str) -> f64 {
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
                store: "amazon",
                title: unescape(&TITLE.captures(card)?[1]),
                price_pen: price(&PRICE.captures(card)?[1]),
                url: format!("https://www.amazon.com{}", &HREF.captures(card)?[1]),
                seller: None,
                rating: RATING.captures(card).map(|c| price(&c[1])),
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
                store: "aliexpress",
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
                store: "falabella",
                title: format!("{} {}", x["brand"].as_str().unwrap_or(""), x["displayName"].as_str()?),
                price_pen: price(x["prices"][0]["price"][0].as_str()?),
                url: x["url"].as_str()?.to_string(),
                seller: x["sellerName"].as_str().map(String::from),
                rating: None,
                sold: None,
            })
        })
        .collect()
}

fn parse_plazavea(json: &str) -> Vec<Listing> {
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
                store: "plazavea",
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
        assert_eq!(price("PEN 1,234.56"), 1234.56);
        assert_eq!(price("S/ 1,299"), 1299.0);
        assert_eq!(price("PEN 205,97"), 205.97);
        assert_eq!(price("310"), 310.0);
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
    fn parses_aliexpress_item_list() {
        let html = r#"..."itemList":{"content":[{"productId":"100","title":{"displayTitle":"FiiO JT3"},
            "prices":{"salePrice":{"minPrice":202.08}},"evaluation":{"starRating":4.9},"trade":{"tradeDesc":"46 vendidos"}}],"x":1}"#;
        let l = &parse_aliexpress(html)[0];
        assert_eq!((l.price_pen, l.sold.as_deref()), (202.08, Some("46 vendidos")));
    }
}
