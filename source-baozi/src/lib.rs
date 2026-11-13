#![no_std]

use koma_source_sdk::host;
use koma_source_sdk::json_utils::{
    append_json_escaped, extract_json_string, find_subslice, json_object_member,
    write_bytes, write_url_encoded, write_usize, JsonArrayIter,
};
use koma_source_sdk::source::{SourceCapabilities, SourceInfo};

const SITE: &[u8] = b"https://baozimh.org";
const API: &[u8] = b"https://api-get-v2.mgsearcher.com";
const IMAGE_CDN: &[u8] = b"https://f40-1-4.g-mh.online";

const SOURCE_INFO: SourceInfo = SourceInfo {
    id: "com.baozimh.koma",
    name: "包子漫画",
    language: "zh-Hans",
    version: env!("CARGO_PKG_VERSION"),
    api_version: "0.2",
    description: "包子漫画公开漫画源",
    author: "rumanhua-koma",
    content_rating: "unknown",
};

const SOURCE_CAPS: SourceCapabilities = SourceCapabilities {
    search: true,
    manga_detail: true,
    chapters: true,
    pages: true,
    listings: true,
    manga_list: true,
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

fn get(url: &[u8]) -> Result<usize, ()> {
    fetch_get(url, Some(SITE)).map_err(|_| ())
}

fn decode_json(raw: &[u8], out: &mut [u8]) -> Result<usize, ()> {
    koma_source_sdk::plain_text::decode_json_content(raw, out)
        .map(|v| v.len()).map_err(|_| ())
}

fn nth<'a>(base: &[u8], index: usize, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
    let mut n = 0;
    if !write_bytes(out, &mut n, base)
        || !write_bytes(out, &mut n, b":nth-of-type(")
        || !write_usize(out, &mut n, index)
        || !write_bytes(out, &mut n, b")") {
        return Err(());
    }
    Ok(&out[..n])
}

fn absolute_url<'a>(raw: &[u8], out: &'a mut [u8]) -> &'a [u8] {
    let mut n = 0;
    if raw.starts_with(b"https://") || raw.starts_with(b"http://") {
        let _ = write_bytes(out, &mut n, raw);
    } else if raw.starts_with(b"//") {
        let _ = write_bytes(out, &mut n, b"https:");
        let _ = write_bytes(out, &mut n, raw);
    } else {
        let _ = write_bytes(out, &mut n, IMAGE_CDN);
        if !raw.starts_with(b"/") { let _ = write_bytes(out, &mut n, b"/"); }
        let _ = write_bytes(out, &mut n, raw);
    }
    &out[..n]
}

fn decimal(raw: &[u8]) -> Option<usize> {
    let v = trim_ascii(raw);
    if v.is_empty() || !v.iter().all(u8::is_ascii_digit) { return None; }
    let mut n = 0usize;
    for b in v {
        n = n.checked_mul(10)?.checked_add((b - b'0') as usize)?;
    }
    Some(n)
}

fn last_path_segment(raw: &[u8]) -> &[u8] {
    let v = raw.split(|b| *b == b'?').next().unwrap_or(raw);
    let v = v.strip_suffix(b"/").unwrap_or(v);
    v.rsplit(|b| *b == b'/').next().unwrap_or(v)
}

fn parse_manga_identity<'a>(raw: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
    let v = raw.strip_prefix(b"manga:")?;
    let split = find_subslice(v, b":")?;
    Some((&v[..split], &v[split + 1..]))
}

fn parse_chapter_identity<'a>(raw: &'a [u8]) -> Option<(&'a [u8], &'a [u8], &'a [u8])> {
    let v = raw.strip_prefix(b"chapter:")?;
    let a = find_subslice(v, b":")?;
    let rest = &v[a + 1..];
    let b = find_subslice(rest, b":")?;
    Some((&v[..a], &rest[..b], &rest[b + 1..]))
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

fn parse_listing_page(url: &[u8], operation: &'static str) -> u32 {
    let hn = match get(url) {
        Ok(n) => n,
        Err(_) => return write_error(operation, "network_error", "listing request failed"),
    };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) {
        Ok(v) => v,
        Err(_) => return write_error(operation, "parse_error", "invalid listing HTML"),
    };
    let p = payload_buf();
    let mut c = 0;
    if !write_bytes(p, &mut c, br#"{"items":["#) {
        return write_error(operation, "internal_error", "payload overflow");
    }
    let mut count = 0;
    for i in 1..=80 {
        let mut selector = [0u8; 128];
        let sel = match nth(b".pb-2", i, &mut selector) {
            Ok(v) => v, Err(_) => break,
        };
        let card = match doc.select(sel) {
            Ok(v) => v, Err(_) => continue,
        };
        let link = match card.select(b"a") {
            Ok(v) => v, Err(_) => continue,
        };
        let mut href = [0u8; 512];
        let raw_href = link.attr(b"href", &mut href).unwrap_or(b"");
        let slug = last_path_segment(raw_href);
        if slug.is_empty() { continue; }

        let mut title = [0u8; 1024];
        let title_text = card.select(b"h3").ok()
            .and_then(|v| v.text(&mut title).ok()).unwrap_or(b"");
        if title_text.is_empty() { continue; }

        let mut image = [0u8; 2048];
        let img_raw = card.select(b"img").ok()
            .and_then(|v| v.attr(b"src", &mut image).ok());
        let mut cover = [0u8; 4096];
        let cover_url = absolute_url(img_raw.unwrap_or(b""), &mut cover);

        if count > 0 && !write_bytes(p, &mut c, b",") {
            return write_error(operation, "internal_error", "payload overflow");
        }
        if !emit_manga_item(p, &mut c, slug, title_text, cover_url) {
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
    let raw = match extract_json_string(req, b"query") {
        Some(v) => v,
        None => return write_error("search", "invalid_request", "missing query"),
    };
    let mut query = [0u8; 2048];
    let qn = match decode_json(raw, &mut query) {
        Ok(n) if n > 0 => n,
        _ => return write_error("search", "invalid_request", "empty query"),
    };
    let mut url = [0u8; 8192];
    let mut n = 0;
    if !write_bytes(&mut url, &mut n, SITE)
        || !write_bytes(&mut url, &mut n, b"/s/")
        || !write_url_encoded(&mut url, &mut n, &query[..qn])
        || !write_bytes(&mut url, &mut n, b"?page=1") {
        return write_error("search", "internal_error", "URL overflow");
    }
    parse_listing_page(&url[..n], "search")
}

fn run_get_manga(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"mangaId") {
        Some(v) => v, None => return write_error("get_manga", "invalid_request", "missing mangaId"),
    };
    let mut id = [0u8; 512];
    let id_len = match decode_json(raw, &mut id) {
        Ok(n) => n, Err(_) => return write_error("get_manga", "invalid_request", "invalid mangaId"),
    };
    let slug = id[..id_len].strip_prefix(b"manga:").unwrap_or(&id[..id_len]);
    let slug = slug.split(|b| *b == b':').next().unwrap_or(slug);
    if slug.is_empty() { return write_error("get_manga", "invalid_request", "empty manga slug"); }

    let mut url = [0u8; 1024];
    let mut n = 0;
    if !write_bytes(&mut url, &mut n, SITE)
        || !write_bytes(&mut url, &mut n, b"/manga/")
        || !write_bytes(&mut url, &mut n, slug) {
        return write_error("get_manga", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..n]) {
        Ok(v) => v, Err(_) => return write_error("get_manga", "network_error", "detail request failed"),
    };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) {
        Ok(v) => v, Err(_) => return write_error("get_manga", "parse_error", "invalid detail HTML"),
    };

    let mut mid_buf = [0u8; 128];
    let mid_raw = doc.select(b"#mangachapters").ok()
        .and_then(|v| v.attr(b"data-mid", &mut mid_buf).ok()).unwrap_or(b"");
    let mid = match decimal(mid_raw) {
        Some(v) if v > 0 => v,
        _ => return write_error("get_manga", "parse_error", "manga data-mid not found"),
    };

    let mut title_buf = [0u8; 2048];
    let mut title = doc.select(b"h1").ok()
        .and_then(|v| v.text(&mut title_buf).ok()).unwrap_or(b"");
    if title.is_empty() {
        title = doc.select(b"title").ok()
            .and_then(|v| v.text(&mut title_buf).ok()).unwrap_or(b"");
    }
    if title.is_empty() { return write_error("get_manga", "parse_error", "manga title not found"); }

    let mut cover_buf = [0u8; 2048];
    let cover_raw = doc.select(b"meta[property='og:image']").ok()
        .and_then(|v| v.attr(b"content", &mut cover_buf).ok()).unwrap_or(b"");
    let mut cover_abs = [0u8; 4096];
    let cover = if cover_raw.starts_with(b"http") { cover_raw }
        else { absolute_url(cover_raw, &mut cover_abs) };

    let mut desc_buf = [0u8; 8192];
    let desc = doc.select(b".text-medium.my-unit-md").ok()
        .and_then(|v| v.text(&mut desc_buf).ok()).unwrap_or(b"");

    let mut author_buf = [0u8; 1024];
    let author = doc.select(b"a[href*='author'] span").ok()
        .and_then(|v| v.text(&mut author_buf).ok()).unwrap_or(b"");

    let p = payload_buf();
    let mut c = 0;
    let mut final_id = [0u8; 512];
    let mut idc = 0;
    if !write_bytes(&mut final_id, &mut idc, b"manga:")
        || !write_bytes(&mut final_id, &mut idc, slug)
        || !write_bytes(&mut final_id, &mut idc, b":")
        || !write_usize(&mut final_id, &mut idc, mid) {
        return write_error("get_manga", "internal_error", "manga ID overflow");
    }
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

static mut CHAPTER_IDS: [[u8; 32]; 2048] = [[0; 32]; 2048];
static mut CHAPTER_TITLES: [[u8; 1024]; 2048] = [[0; 1024]; 2048];
static mut CHAPTER_ID_LENS: [usize; 2048] = [0; 2048];
static mut CHAPTER_TITLE_LENS: [usize; 2048] = [0; 2048];

fn run_get_chapters(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"mangaId") {
        Some(v) => v, None => return write_error("get_chapters", "invalid_request", "missing mangaId"),
    };
    let mut id = [0u8; 512];
    let id_len = match decode_json(raw, &mut id) {
        Ok(n) => n, Err(_) => return write_error("get_chapters", "invalid_request", "invalid mangaId"),
    };
    let (slug, mid_raw) = match parse_manga_identity(&id[..id_len]) {
        Some(v) => v, None => return write_error("get_chapters", "invalid_request", "manga detail must be loaded first"),
    };
    let mid = match decimal(mid_raw) {
        Some(v) if v > 0 => v, _ => return write_error("get_chapters", "invalid_request", "invalid manga ID"),
    };
    let mut url = [0u8; 1024];
    let mut n = 0;
    if !write_bytes(&mut url, &mut n, API)
        || !write_bytes(&mut url, &mut n, b"/api/manga/get?mid=")
        || !write_bytes(&mut url, &mut n, mid_raw)
        || !write_bytes(&mut url, &mut n, b"&mode=all") {
        return write_error("get_chapters", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..n]) {
        Ok(v) => v, Err(_) => return write_error("get_chapters", "network_error", "chapter API request failed"),
    };
    let response = &body_buf()[..hn];
    let data = match json_object_member(response, b"data") {
        Some(v) => v, None => return write_error("get_chapters", "parse_error", "chapter API data missing"),
    };
    let array = match json_object_member(data, b"chapters") {
        Some(v) => v, None => return write_error("get_chapters", "parse_error", "chapter array missing"),
    };
    let mut iter = match JsonArrayIter::from_raw_array(array) {
        Some(v) => v, None => return write_error("get_chapters", "parse_error", "invalid chapter array"),
    };
    unsafe {
        CHAPTER_ID_LENS = [0; 2048];
        CHAPTER_TITLE_LENS = [0; 2048];
    }
    let mut count = 0usize;
    while let Some(obj) = iter.next_object() {
        if count >= 2048 { break; }
        let chapter_raw = match json_object_member(obj, b"id") {
            Some(v) => v, None => continue,
        };
        let chapter_id = trim_ascii(chapter_raw);
        let chapter_id = chapter_id.strip_prefix(b"\"").and_then(|v| v.strip_suffix(b"\"")).unwrap_or(chapter_id);
        if chapter_id.is_empty() || chapter_id.len() >= 32 { continue; }
        let attrs = match json_object_member(obj, b"attributes") {
            Some(v) => v, None => continue,
        };
        let title_raw = match extract_json_string(attrs, b"title") { Some(v) => v, None => continue };
        let slug_raw = match extract_json_string(attrs, b"slug") { Some(v) => v, None => continue };
        let mut title = [0u8; 1024];
        let tn = match decode_json(title_raw, &mut title) { Ok(v) if v > 0 => v, _ => continue };
        let mut chapter_slug = [0u8; 512];
        let sn = match decode_json(slug_raw, &mut chapter_slug) { Ok(v) if v > 0 => v, _ => continue };
        // Keep a stable chapter identity while retaining the site's own chapter slug.
        unsafe {
            CHAPTER_IDS[count][..chapter_id.len()].copy_from_slice(chapter_id);
            CHAPTER_ID_LENS[count] = chapter_id.len();
            CHAPTER_TITLES[count][..tn].copy_from_slice(&title[..tn]);
            CHAPTER_TITLE_LENS[count] = tn;
        }
        // The slug is stored in the emitted URL fragment, not in the chapter ID.
        let _ = sn;
        count += 1;
    }
    if count == 0 { return write_error("get_chapters", "parse_error", "no public chapters found"); }

    let p = payload_buf();
    let mut c = 0;
    if !write_bytes(p, &mut c, br#"{"items":["#) {
        return write_error("get_chapters", "internal_error", "payload overflow");
    }
    for i in 0..count {
        let idx = count - 1 - i;
        let mut chapter_identity = [0u8; 512];
        let mut ci = 0;
        if !write_bytes(&mut chapter_identity, &mut ci, b"chapter:")
            || !write_bytes(&mut chapter_identity, &mut ci, slug)
            || !write_bytes(&mut chapter_identity, &mut ci, b":")
            || !write_bytes(&mut chapter_identity, &mut ci, mid_raw)
            || !write_bytes(&mut chapter_identity, &mut ci, b":")
            || !write_bytes(&mut chapter_identity, &mut ci, unsafe { &CHAPTER_IDS[idx][..CHAPTER_ID_LENS[idx]] }) {
            return write_error("get_chapters", "internal_error", "chapter ID overflow");
        }
        if i > 0 && !write_bytes(p, &mut c, b",") {
            return write_error("get_chapters", "internal_error", "payload overflow");
        }
        let ok = write_bytes(p, &mut c, br#"{"id":""#)
            && append_json_escaped(p, &mut c, &chapter_identity[..ci])
            && write_bytes(p, &mut c, br#"","mangaId":""#)
            && append_json_escaped(p, &mut c, &id[..id_len])
            && write_bytes(p, &mut c, br#"","title":""#)
            && append_json_escaped(p, &mut c, unsafe { &CHAPTER_TITLES[idx][..CHAPTER_TITLE_LENS[idx]] })
            && write_bytes(p, &mut c, br#"","language":"zh-Hans"}"#);
        if !ok { return write_error("get_chapters", "internal_error", "payload overflow"); }
    }
    if !write_bytes(p, &mut c, br#"],"page":{"nextCursor":null,"hasMore":false}}"#) {
        return write_error("get_chapters", "internal_error", "payload overflow");
    }
    write_success_payload("get_chapters", c)
}

fn run_get_pages(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"chapterId") {
        Some(v) => v, None => return write_error("get_pages", "invalid_request", "missing chapterId"),
    };
    let mut id = [0u8; 512];
    let id_len = match decode_json(raw, &mut id) {
        Ok(n) => n, Err(_) => return write_error("get_pages", "invalid_request", "invalid chapterId"),
    };
    let (_slug, mid_raw, chapter_raw) = match parse_chapter_identity(&id[..id_len]) {
        Some(v) => v, None => return write_error("get_pages", "invalid_request", "invalid chapter ID"),
    };
    let mut url = [0u8; 1024];
    let mut n = 0;
    if !write_bytes(&mut url, &mut n, API)
        || !write_bytes(&mut url, &mut n, b"/api/chapter/getinfo?m=")
        || !write_bytes(&mut url, &mut n, mid_raw)
        || !write_bytes(&mut url, &mut n, b"&c=")
        || !write_bytes(&mut url, &mut n, chapter_raw) {
        return write_error("get_pages", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..n]) {
        Ok(v) => v, Err(_) => return write_error("get_pages", "network_error", "reader API request failed"),
    };
    let response = &body_buf()[..hn];
    let data = match json_object_member(response, b"data") {
        Some(v) => v, None => return write_error("get_pages", "parse_error", "reader API data missing"),
    };
    let info = match json_object_member(data, b"info") {
        Some(v) => v, None => return write_error("get_pages", "parse_error", "reader info missing"),
    };
    let images_obj = match json_object_member(info, b"images") {
        Some(v) => v, None => return write_error("get_pages", "parse_error", "image data missing"),
    };
    let images = match json_object_member(images_obj, b"images") {
        Some(v) => v, None => return write_error("get_pages", "parse_error", "image array missing"),
    };
    let mut iter = match JsonArrayIter::from_raw_array(images) {
        Some(v) => v, None => return write_error("get_pages", "parse_error", "invalid image array"),
    };

    let p = payload_buf();
    let mut c = 0;
    let mut count = 0usize;
    if !write_bytes(p, &mut c, br#"{"chapterId":""#)
        || !append_json_escaped(p, &mut c, &id[..id_len])
        || !write_bytes(p, &mut c, br#"","pages":["#) {
        return write_error("get_pages", "internal_error", "payload overflow");
    }
    while let Some(obj) = iter.next_object() {
        let raw_url = match extract_json_string(obj, b"url") {
            Some(v) => v, None => continue,
        };
        let mut decoded = [0u8; 2048];
        let un = match decode_json(raw_url, &mut decoded) {
            Ok(v) if v > 0 => v, _ => continue,
        };
        let mut image = [0u8; 4096];
        let image_url = absolute_url(&decoded[..un], &mut image);
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
    if count == 0 { return write_error("get_pages", "parse_error", "no public page images found"); }
    if !write_bytes(p, &mut c, b"]}") {
        return write_error("get_pages", "internal_error", "payload overflow");
    }
    write_success_payload("get_pages", c)
}

fn run_get_listings(_req: &[u8]) -> u32 {
    let data = r#"{"listings":[{"id":"popular","name":"人气推荐"},{"id":"dayup","name":"热门更新"},{"id":"newss","name":"最新上架"}]}"#.as_bytes();
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_listings", data.len())
}

fn run_get_manga_list(req: &[u8]) -> u32 {
    let listing_raw = extract_json_string(req, b"listingId").unwrap_or(b"newss");
    let mut listing = [0u8; 64];
    let ln = match decode_json(listing_raw, &mut listing) {
        Ok(v) => v, Err(_) => return write_error("get_manga_list", "invalid_request", "invalid listing ID"),
    };
    let route: &[u8] = match &listing[..ln] {
        b"popular" => b"/hots/page/1",
        b"dayup" => b"/dayup/page/1",
        b"newss" => b"/newss/page/1",
        _ => b"/manga/page/1",
    };
    let mut url = [0u8; 512];
    let mut n = 0;
    if !write_bytes(&mut url, &mut n, SITE) || !write_bytes(&mut url, &mut n, route) {
        return write_error("get_manga_list", "internal_error", "URL overflow");
    }
    parse_listing_page(&url[..n], "get_manga_list")
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

fn run_get_settings(_req: &[u8]) -> u32 {
    let data = br#"{"settings":[]}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_settings", data.len())
}

fn run_get_image_request(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"url") {
        Some(v) => v, None => return write_error("get_image_request", "invalid_request", "missing image URL"),
    };
    let p = payload_buf();
    let mut c = 0;
    let ok = write_bytes(p, &mut c, br#"{"url":""#)
        && append_json_escaped(p, &mut c, raw)
        && write_bytes(p, &mut c, br#"","headers":{"Referer":"https://baozimh.org/","User-Agent":"Mozilla/5.0"}}"#);
    if !ok { return write_error("get_image_request", "internal_error", "payload overflow"); }
    write_success_payload("get_image_request", c)
}

koma_source_sdk::koma_source_exports!("baozimh");
