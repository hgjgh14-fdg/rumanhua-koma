#![no_std]

use koma_source_sdk::host;
use koma_source_sdk::json_utils::{
    append_json_escaped, extract_json_string, find_subslice, json_object_member,
    write_bytes, write_url_encoded, write_usize,
};
use koma_source_sdk::source::{SourceCapabilities, SourceInfo};

const SITES: [&[u8]; 6] = [
    b"https://manwapr.cc",
    b"https://manwass.cc",
    b"https://manwatg.cc",
    b"https://manwa.me",
    b"https://manwast.cc",
    b"https://manwasy.cc",
];
const MOBILE_UA: &[u8] = b"Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
static mut ACTIVE_SITE: &[u8] = SITES[0];

const SOURCE_INFO: SourceInfo = SourceInfo {
    id: "com.manwa.koma",
    name: "漫蛙",
    language: "zh-Hans",
    version: env!("CARGO_PKG_VERSION"),
    api_version: "0.2",
    description: "漫蛙公开漫画源",
    author: "rumanhua-koma",
    content_rating: "unknown",
};

const SOURCE_CAPS: SourceCapabilities = SourceCapabilities {
    search: true,
    manga_detail: true,
    chapters: true,
    pages: true,
    listings: false,
    manga_list: false,
    filters: false,
    settings: true,
    home: false,
    credentials: false,
    image_request: true,
};

koma_source_sdk::koma_source_buffers! {
    payload: 1024 * 1024,
    http_out: 3 * 1024 * 1024,
    body: 3 * 1024 * 1024,
    http_req: 8192,
    scratch: 16384,
}
koma_source_sdk::koma_source_helpers!();

fn site() -> &'static [u8] {
    unsafe { ACTIVE_SITE }
}

fn homepage_choices_supported(req: &[u8]) -> bool {
    json_object_member(req, b"hostFeatures")
        .map(|v| {
            v.starts_with(b"[")
                && v.ends_with(b"]")
                && v[1..v.len() - 1]
                    .split(|b| *b == b',')
                    .any(|item| item.trim_ascii() == br#""source_homepage_choices_v1""#)
        })
        .unwrap_or(false)
}

fn get(url: &[u8]) -> Result<usize, ()> {
    let mut referer = [0u8; 256];
    let mut rn = 0usize;
    if !write_bytes(&mut referer, &mut rn, site())
        || !write_bytes(&mut referer, &mut rn, b"/") {
        return Err(());
    }

    let req = http_req_buf();
    let mut n = 0usize;
    let ok = write_bytes(req, &mut n, br#"{"version":1,"method":"GET","url":""#)
        && append_json_escaped(req, &mut n, url)
        && write_bytes(req, &mut n, br#"","headers":{"User-Agent":""#)
        && append_json_escaped(req, &mut n, MOBILE_UA)
        && write_bytes(req, &mut n, br#"","Referer":""#)
        && append_json_escaped(req, &mut n, &referer[..rn])
        && write_bytes(req, &mut n, br#"","Pragma":"no-cache"},"timeoutMs":15000,"responseKind":"bodyText"}"#);
    if !ok { return Err(()); }
    let response_len = host::http_request(&req[..n], http_out()).map_err(|_| ())?;
    koma_source_sdk::decode_json_body_into(&http_out()[..response_len], body_buf())
        .map_err(|_| ())
}

fn select_site(req: &[u8]) {
    if homepage_choices_supported(req) {
        let selected = json_object_member(req, b"settings")
            .and_then(|settings| extract_json_string(settings, b"preferredMirror"));
        let chosen = selected
            .and_then(|value| SITES.iter().copied().find(|candidate| value == *candidate))
            .unwrap_or(SITES[0]);
        unsafe { ACTIVE_SITE = chosen; }
        return;
    }

    for candidate in SITES.iter() {
        unsafe { ACTIVE_SITE = *candidate; }
        let mut url = [0u8; 512];
        let mut n = 0usize;
        if write_bytes(&mut url, &mut n, candidate)
            && write_bytes(&mut url, &mut n, b"/")
            && get(&url[..n]).is_ok() {
            return;
        }
    }
    unsafe { ACTIVE_SITE = SITES[0]; }
}

struct HtmlNode(host::HtmlDescriptor);
impl Drop for HtmlNode {
    fn drop(&mut self) { let _ = host::html_close(self.0); }
}
impl HtmlNode {
    fn parse(html: &[u8]) -> Result<Self, ()> {
        host::html_parse(html).map(Self).map_err(|_| ())
    }
    fn select(&self, selector: &[u8]) -> Result<Self, ()> {
        host::html_select(self.0, selector).map(Self).map_err(|_| ())
    }
    fn attr<'a>(&self, key: &[u8], out: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let n = host::html_attr(self.0, key, out).map_err(|_| ())?;
        if n > out.len() { return Err(()); }
        Ok(&out[..n])
    }
    fn text<'a>(&self, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let n = host::html_text(self.0, out).map_err(|_| ())?;
        if n > out.len() { return Err(()); }
        Ok(trim_ascii(&out[..n]))
    }
}

fn nth<'a>(base: &[u8], index: usize, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
    let mut n = 0usize;
    if !write_bytes(out, &mut n, base)
        || !write_bytes(out, &mut n, b":nth-of-type(")
        || !write_usize(out, &mut n, index)
        || !write_bytes(out, &mut n, b")") {
        return Err(());
    }
    Ok(&out[..n])
}

fn absolute_url<'a>(raw: &[u8], out: &'a mut [u8]) -> &'a [u8] {
    let mut n = 0usize;
    if raw.starts_with(b"https://") || raw.starts_with(b"http://") {
        let _ = write_bytes(out, &mut n, raw);
    } else if raw.starts_with(b"//") {
        let _ = write_bytes(out, &mut n, b"https:");
        let _ = write_bytes(out, &mut n, raw);
    } else {
        let _ = write_bytes(out, &mut n, site());
        if !raw.starts_with(b"/") { let _ = write_bytes(out, &mut n, b"/"); }
        let _ = write_bytes(out, &mut n, raw);
    }
    &out[..n]
}

fn last_path_segment(raw: &[u8]) -> &[u8] {
    let v = raw.split(|b| *b == b'?').next().unwrap_or(raw);
    let v = v.strip_suffix(b"/").unwrap_or(v);
    v.rsplit(|b| *b == b'/').next().unwrap_or(v)
}

fn decode_json(raw: &[u8], out: &mut [u8]) -> Result<usize, ()> {
    koma_source_sdk::plain_text::decode_json_content(raw, out)
        .map(|v| v.len()).map_err(|_| ())
}

fn emit_manga_item(
    out: &mut [u8], cursor: &mut usize, slug: &[u8], title: &[u8], cover: &[u8],
) -> bool {
    write_bytes(out, cursor, br#"{"id":"manga:"#)
        && append_json_escaped(out, cursor, slug)
        && write_bytes(out, cursor, br#"","title":""#)
        && append_json_escaped(out, cursor, title)
        && write_bytes(out, cursor, br#"","cover":{"kind":"url","url":""#)
        && append_json_escaped(out, cursor, cover)
        && write_bytes(out, cursor, br#""},"authors":[],"status":"unknown","contentRating":"unknown"}"#)
}

fn parse_search_page(url: &[u8], operation: &'static str) -> u32 {
    let hn = match get(url) {
        Ok(v) => v,
        Err(_) => return write_error(operation, "network_error", "Manwa search request failed"),
    };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) {
        Ok(v) => v,
        Err(_) => return write_error(operation, "parse_error", "invalid Manwa search HTML"),
    };

    let p = payload_buf();
    let mut c = 0usize;
    if !write_bytes(p, &mut c, br#"{"items":["#) {
        return write_error(operation, "internal_error", "payload overflow");
    }
    let mut count = 0usize;
    for i in 1..=100 {
        let mut selector = [0u8; 160];
        let sel = match nth(b"ul.book-list > li", i, &mut selector) {
            Ok(v) => v, Err(_) => break,
        };
        let item = match doc.select(sel) {
            Ok(v) => v, Err(_) => continue,
        };
        let mut title_buf = [0u8; 1024];
        let title = item.select(b"p.book-list-info-title").ok()
            .and_then(|v| v.text(&mut title_buf).ok()).unwrap_or(b"");
        if title.is_empty() { continue; }

        let mut href_buf = [0u8; 1024];
        let href = item.select(b"a").ok()
            .and_then(|v| v.attr(b"href", &mut href_buf).ok()).unwrap_or(b"");
        let slug = last_path_segment(href);
        if slug.is_empty() { continue; }

        let img = match item.select(b"img") {
            Ok(v) => v, Err(_) => continue,
        };
        let mut cover_original = [0u8; 2048];
        let mut cover_src = [0u8; 2048];
        let original = img.attr(b"data-original", &mut cover_original).unwrap_or(b"");
        let cover_raw = if original.is_empty() {
            img.attr(b"src", &mut cover_src).unwrap_or(b"")
        } else { original };
        let mut cover_abs = [0u8; 4096];
        let cover = absolute_url(cover_raw, &mut cover_abs);

        if count > 0 && !write_bytes(p, &mut c, b",") {
            return write_error(operation, "internal_error", "payload overflow");
        }
        if !emit_manga_item(p, &mut c, slug, title, cover) {
            return write_error(operation, "internal_error", "payload overflow");
        }
        count += 1;
    }
    if !write_bytes(p, &mut c, br#"],"page":{"nextCursor":null,"hasMore":false}}"#) {
        return write_error(operation, "internal_error", "payload overflow");
    }
    write_success_payload(operation, c)
}

fn run_search(req: &[u8]) -> u32 {
    select_site(req);
    let raw = match extract_json_string(req, b"query") {
        Some(v) => v,
        None => return write_error("search", "invalid_request", "missing query"),
    };
    let mut query = [0u8; 2048];
    let qn = match decode_json(raw, &mut query) {
        Ok(v) if v > 0 => v,
        _ => return write_error("search", "invalid_request", "empty query"),
    };
    let mut url = [0u8; 8192];
    let mut n = 0usize;
    if !write_bytes(&mut url, &mut n, site())
        || !write_bytes(&mut url, &mut n, b"/search?keyword=")
        || !write_url_encoded(&mut url, &mut n, &query[..qn])
        || !write_bytes(&mut url, &mut n, b"&page=1") {
        return write_error("search", "internal_error", "URL overflow");
    }
    parse_search_page(&url[..n], "search")
}

fn manga_slug<'a>(raw: &'a [u8]) -> Option<&'a [u8]> {
    let value = raw.strip_prefix(b"manga:").unwrap_or(raw);
    let slug = value.split(|b| *b == b':').next().unwrap_or(value);
    if slug.is_empty() { None } else { Some(slug) }
}

fn run_get_manga(req: &[u8]) -> u32 {
    select_site(req);
    let raw = match extract_json_string(req, b"mangaId") {
        Some(v) => v, None => return write_error("get_manga", "invalid_request", "missing mangaId"),
    };
    let mut id = [0u8; 512];
    let id_len = match decode_json(raw, &mut id) {
        Ok(v) => v, Err(_) => return write_error("get_manga", "invalid_request", "invalid mangaId"),
    };
    let slug = match manga_slug(&id[..id_len]) {
        Some(v) => v, None => return write_error("get_manga", "invalid_request", "empty manga slug"),
    };
    let mut url = [0u8; 1024];
    let mut n = 0usize;
    if !write_bytes(&mut url, &mut n, site())
        || !write_bytes(&mut url, &mut n, b"/book/")
        || !write_bytes(&mut url, &mut n, slug) {
        return write_error("get_manga", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..n]) {
        Ok(v) => v, Err(_) => return write_error("get_manga", "network_error", "Manwa detail request failed"),
    };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) {
        Ok(v) => v, Err(_) => return write_error("get_manga", "parse_error", "invalid Manwa detail HTML"),
    };

    let mut title_buf = [0u8; 2048];
    let title = doc.select(b".detail-main-info-title").ok()
        .and_then(|v| v.text(&mut title_buf).ok()).unwrap_or(b"");
    if title.is_empty() { return write_error("get_manga", "parse_error", "Manwa title not found"); }

    let mut cover_original = [0u8; 2048];
    let mut cover_src = [0u8; 2048];
    let cover_node = doc.select(b"div.detail-main-cover > img").ok();
    let cover_raw = if let Some(img) = cover_node {
        let original = img.attr(b"data-original", &mut cover_original).unwrap_or(b"");
        if original.is_empty() { img.attr(b"src", &mut cover_src).unwrap_or(b"") } else { original }
    } else { b"" };
    let mut cover_abs = [0u8; 4096];
    let cover = absolute_url(cover_raw, &mut cover_abs);

    let mut desc_buf = [0u8; 8192];
    let desc = doc.select(b"#detail > p.detail-desc").ok()
        .and_then(|v| v.text(&mut desc_buf).ok()).unwrap_or(b"");

    let mut author_buf = [0u8; 1024];
    let author = doc.select(b"p.detail-main-info-author > span.detail-main-info-value:nth-of-type(2)").ok()
        .and_then(|v| v.text(&mut author_buf).ok()).unwrap_or(b"");

    let mut final_id = [0u8; 512];
    let mut idc = 0usize;
    if !write_bytes(&mut final_id, &mut idc, b"manga:")
        || !write_bytes(&mut final_id, &mut idc, slug) {
        return write_error("get_manga", "internal_error", "manga ID overflow");
    }

    let p = payload_buf();
    let mut c = 0usize;
    let ok = write_bytes(p, &mut c, br#"{"manga":{"id":""#)
        && append_json_escaped(p, &mut c, &final_id[..idc])
        && write_bytes(p, &mut c, br#"","title":""#)
        && append_json_escaped(p, &mut c, title)
        && write_bytes(p, &mut c, br#"","description":""#)
        && append_json_escaped(p, &mut c, desc)
        && write_bytes(p, &mut c, br#"","cover":{"kind":"url","url":""#)
        && append_json_escaped(p, &mut c, cover)
        && write_bytes(p, &mut c, br#""},"authors":["#)
        && (author.is_empty() || (write_bytes(p, &mut c, b"\"") && append_json_escaped(p, &mut c, author) && write_bytes(p, &mut c, b"\"")))
        && write_bytes(p, &mut c, br#"],"status":"unknown","language":"zh-Hans","contentRating":"unknown","tags":[],"links":[]}}"#);
    if !ok { return write_error("get_manga", "internal_error", "payload overflow"); }
    write_success_payload("get_manga", c)
}

fn run_get_chapters(req: &[u8]) -> u32 {
    select_site(req);
    let raw = match extract_json_string(req, b"mangaId") {
        Some(v) => v, None => return write_error("get_chapters", "invalid_request", "missing mangaId"),
    };
    let mut id = [0u8; 512];
    let id_len = match decode_json(raw, &mut id) {
        Ok(v) => v, Err(_) => return write_error("get_chapters", "invalid_request", "invalid mangaId"),
    };
    let slug = match manga_slug(&id[..id_len]) {
        Some(v) => v, None => return write_error("get_chapters", "invalid_request", "empty manga slug"),
    };
    let mut url = [0u8; 1024];
    let mut n = 0usize;
    if !write_bytes(&mut url, &mut n, site())
        || !write_bytes(&mut url, &mut n, b"/book/")
        || !write_bytes(&mut url, &mut n, slug) {
        return write_error("get_chapters", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..n]) {
        Ok(v) => v, Err(_) => return write_error("get_chapters", "network_error", "Manwa detail request failed"),
    };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) {
        Ok(v) => v, Err(_) => return write_error("get_chapters", "parse_error", "invalid Manwa detail HTML"),
    };

    let p = payload_buf();
    let mut c = 0usize;
    if !write_bytes(p, &mut c, br#"{"items":["#) {
        return write_error("get_chapters", "internal_error", "payload overflow");
    }
    let mut count = 0usize;
    for i in 1..=1000 {
        let mut selector = [0u8; 192];
        let sel = match nth(b"ul#detail-list-select > li", i, &mut selector) {
            Ok(v) => v, Err(_) => break,
        };
        let li = match doc.select(sel) {
            Ok(v) => v, Err(_) => continue,
        };
        let link = match li.select(b"a") {
            Ok(v) => v, Err(_) => continue,
        };
        let mut href_buf = [0u8; 1024];
        let href = link.attr(b"href", &mut href_buf).unwrap_or(b"");
        let chapter_slug = last_path_segment(href);
        if chapter_slug.is_empty() { continue; }
        let mut title_buf = [0u8; 1024];
        let title = link.text(&mut title_buf).unwrap_or(b"");
        if title.is_empty() { continue; }

        let mut chapter_id = [0u8; 512];
        let mut cn = 0usize;
        if !write_bytes(&mut chapter_id, &mut cn, b"chapter:")
            || !write_bytes(&mut chapter_id, &mut cn, slug)
            || !write_bytes(&mut chapter_id, &mut cn, b":")
            || !write_bytes(&mut chapter_id, &mut cn, chapter_slug) {
            return write_error("get_chapters", "internal_error", "chapter ID overflow");
        }
        if count > 0 && !write_bytes(p, &mut c, b",") {
            return write_error("get_chapters", "internal_error", "payload overflow");
        }
        let ok = write_bytes(p, &mut c, br#"{"id":""#)
            && append_json_escaped(p, &mut c, &chapter_id[..cn])
            && write_bytes(p, &mut c, br#"","mangaId":""#)
            && append_json_escaped(p, &mut c, &id[..id_len])
            && write_bytes(p, &mut c, br#"","title":""#)
            && append_json_escaped(p, &mut c, title)
            && write_bytes(p, &mut c, br#"","language":"zh-Hans"}"#);
        if !ok { return write_error("get_chapters", "internal_error", "payload overflow"); }
        count += 1;
    }
    if count == 0 { return write_error("get_chapters", "parse_error", "no Manwa chapters found"); }
    if !write_bytes(p, &mut c, br#"],"page":{"nextCursor":null,"hasMore":false}}"#) {
        return write_error("get_chapters", "internal_error", "payload overflow");
    }
    write_success_payload("get_chapters", c)
}

fn run_get_pages(req: &[u8]) -> u32 {
    select_site(req);
    let raw = match extract_json_string(req, b"chapterId") {
        Some(v) => v, None => return write_error("get_pages", "invalid_request", "missing chapterId"),
    };
    let mut id = [0u8; 512];
    let id_len = match decode_json(raw, &mut id) {
        Ok(v) => v, Err(_) => return write_error("get_pages", "invalid_request", "invalid chapterId"),
    };
    let rest = match id[..id_len].strip_prefix(b"chapter:") {
        Some(v) => v, None => return write_error("get_pages", "invalid_request", "invalid chapter ID"),
    };
    let sep = match find_subslice(rest, b":") {
        Some(v) => v, None => return write_error("get_pages", "invalid_request", "invalid chapter ID"),
    };
    let chapter_slug = &rest[sep + 1..];
    let mut url = [0u8; 1024];
    let mut n = 0usize;
    if !write_bytes(&mut url, &mut n, site())
        || !write_bytes(&mut url, &mut n, b"/chapter/")
        || !write_bytes(&mut url, &mut n, chapter_slug) {
        return write_error("get_pages", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..n]) {
        Ok(v) => v, Err(_) => return write_error("get_pages", "network_error", "Manwa reader request failed"),
    };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) {
        Ok(v) => v, Err(_) => return write_error("get_pages", "parse_error", "invalid Manwa reader HTML"),
    };

    let p = payload_buf();
    let mut c = 0usize;
    if !write_bytes(p, &mut c, br#"{"chapterId":""#)
        || !append_json_escaped(p, &mut c, &id[..id_len])
        || !write_bytes(p, &mut c, br#"","pages":["#) {
        return write_error("get_pages", "internal_error", "payload overflow");
    }
    let mut count = 0usize;
    for i in 1..=1000 {
        let mut selector = [0u8; 256];
        let sel = match nth(b"#cp_img > div.img-content > img[data-r-src]", i, &mut selector) {
            Ok(v) => v, Err(_) => break,
        };
        let img = match doc.select(sel) {
            Ok(v) => v, Err(_) => continue,
        };
        let mut raw_url_buf = [0u8; 2048];
        let raw_url = img.attr(b"data-r-src", &mut raw_url_buf).unwrap_or(b"");
        if raw_url.is_empty() { continue; }
        let mut image = [0u8; 4096];
        let image_url = absolute_url(raw_url, &mut image);
        if count > 0 && !write_bytes(p, &mut c, b",") {
            return write_error("get_pages", "internal_error", "payload overflow");
        }
        let ok = write_bytes(p, &mut c, br#"{"id":"page:"#)
            && write_usize(p, &mut c, count)
            && write_bytes(p, &mut c, br#"","index":"#)
            && write_usize(p, &mut c, count)
            && write_bytes(p, &mut c, br#","image":{"kind":"url","url":""#)
            && append_json_escaped(p, &mut c, image_url)
            && write_bytes(p, &mut c, br#""}}"#);
        if !ok { return write_error("get_pages", "internal_error", "payload overflow"); }
        count += 1;
    }
    if count == 0 { return write_error("get_pages", "parse_error", "no Manwa reader images found"); }
    if !write_bytes(p, &mut c, b"]}") {
        return write_error("get_pages", "internal_error", "payload overflow");
    }
    write_success_payload("get_pages", c)
}

fn run_get_settings(req: &[u8]) -> u32 {
    let data: &[u8] = if homepage_choices_supported(req) {
        r#"{"settings":[{"id":"preferredMirror","name":"访问域名","kind":"select","default":"https://manwapr.cc","description":"漫蛙可用域名。若当前域名无法访问，可在此切换。","options":[{"value":"https://manwapr.cc","label":"manwapr.cc"},{"value":"https://manwass.cc","label":"manwass.cc"},{"value":"https://manwatg.cc","label":"manwatg.cc"},{"value":"https://manwa.me","label":"manwa.me"},{"value":"https://manwast.cc","label":"manwast.cc"},{"value":"https://manwasy.cc","label":"manwasy.cc"}]}"#.as_bytes()
    } else {
        br#"{"settings":[]}"#
    };
    if data.len() > payload_buf().len() { return write_error("get_settings", "internal_error", "settings overflow"); }
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_settings", data.len())
}

fn run_get_image_request(req: &[u8]) -> u32 {
    select_site(req);
    let raw = match extract_json_string(req, b"url") {
        Some(v) => v, None => return write_error("get_image_request", "invalid_request", "missing image URL"),
    };
    let mut referer = [0u8; 256];
    let mut rn = 0usize;
    if !write_bytes(&mut referer, &mut rn, site()) || !write_bytes(&mut referer, &mut rn, b"/") {
        return write_error("get_image_request", "internal_error", "referer overflow");
    }
    let p = payload_buf();
    let mut c = 0usize;
    let ok = write_bytes(p, &mut c, br#"{"url":""#)
        && append_json_escaped(p, &mut c, raw)
        && write_bytes(p, &mut c, br#"","headers":{"Referer":""#)
        && append_json_escaped(p, &mut c, &referer[..rn])
        && write_bytes(p, &mut c, br#"","User-Agent":""#)
        && append_json_escaped(p, &mut c, MOBILE_UA)
        && write_bytes(p, &mut c, br#"","Pragma":"no-cache"}}"#);
    if !ok { return write_error("get_image_request", "internal_error", "payload overflow"); }
    write_success_payload("get_image_request", c)
}

fn run_get_listings(_req: &[u8]) -> u32 {
    let data = br#"{"listings":[]}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_listings", data.len())
}
fn run_get_manga_list(_req: &[u8]) -> u32 {
    let data = br#"{"items":[],"page":{"nextCursor":null,"hasMore":false}}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_manga_list", data.len())
}
fn run_get_home(_req: &[u8]) -> u32 {
    let data = br#"{"sections":[]}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_home", data.len())
}
fn run_get_filters(_req: &[u8]) -> u32 {
    let data = br#"{"filters":[]}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_filters", data.len())
}

koma_source_sdk::koma_source_exports!("manwa");
