#![no_std]

use koma_source_sdk::host;
use koma_source_sdk::json_utils::{
    append_json_escaped, extract_json_string, find_subslice, json_object_member,
    request_page_number, write_bytes, write_url_encoded, write_usize, JsonArrayIter,
};
use koma_source_sdk::source::{SourceCapabilities, SourceInfo};

const BASE: &[u8] = b"https://comic.acgn.cc";
const CSE_CX: &[u8] = b"008124722090293425135:k7bx4juliog";
const CSE_CALLBACK: &[u8] = b"google.search.cse.api1";

const SOURCE_INFO: SourceInfo = SourceInfo {
    id: "com.acgn.koma",
    name: "动漫戏说（ACGN）",
    language: "zh",
    version: env!("CARGO_PKG_VERSION"),
    api_version: "0.2",
    description: "ACGN 动漫戏说公开漫画源",
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
    settings: false,
    home: false,
    credentials: false,
    image_request: true,
};

koma_source_sdk::koma_source_buffers! {
    payload: 1024 * 1024,
    http_out: 3 * 1024 * 1024,
    body: 3 * 1024 * 1024,
    http_req: 16384,
    scratch: 32768,
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

fn get(url: &[u8], referer: Option<&[u8]>) -> Result<usize, ()> {
    fetch_get(url, referer).map_err(|_| ())
}

fn decode(raw: &[u8], out: &mut [u8]) -> Result<usize, ()> {
    koma_source_sdk::plain_text::decode_json_content(raw, out)
        .map(|v| v.len()).map_err(|_| ())
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
        let _ = write_bytes(out, &mut n, BASE);
        if !raw.starts_with(b"/") { let _ = write_bytes(out, &mut n, b"/"); }
        let _ = write_bytes(out, &mut n, raw);
    }
    &out[..n]
}

fn slug_from_url(url: &[u8]) -> Option<&[u8]> {
    let p = find_subslice(url, b"/manhua-")? + 8;
    let end = p + find_subslice(&url[p..], b".htm")?;
    if end <= p { None } else { Some(&url[p..end]) }
}

fn chapter_from_url(url: &[u8]) -> Option<&[u8]> {
    let p = find_subslice(url, b"/view-")? + 6;
    let end = p + find_subslice(&url[p..], b".htm")?;
    if end <= p { None } else { Some(&url[p..end]) }
}

fn write_item(p: &mut [u8], c: &mut usize, id: &[u8], title: &[u8], cover: &[u8]) -> bool {
    write_bytes(p, c, br#"{"id":"manga:"#)
        && append_json_escaped(p, c, id)
        && write_bytes(p, c, br#"","title":""#)
        && append_json_escaped(p, c, title)
        && write_bytes(p, c, br#"","cover":{"kind":"url","url":""#)
        && append_json_escaped(p, c, cover)
        && write_bytes(p, c, br#""},"authors":[],"status":"unknown","contentRating":"unknown","language":"zh"}"#)
}

fn parse_cards(doc: &HtmlNode, operation: &'static str, popular: bool, page: usize) -> u32 {
    let p = payload_buf();
    let mut c = 0usize;
    if !write_bytes(p, &mut c, br#"{"items":["#) {
        return write_error(operation, "internal_error", "payload overflow");
    }
    let mut count = 0usize;
    for i in 1..=60 {
        let mut selector = [0u8; 192];
        let base: &[u8] = if popular { b".tabs-panel li" } else { b"#display .content_block" };
        let sel = match nth(base, i, &mut selector) { Ok(v) => v, Err(_) => break };
        let card = match doc.select(sel) { Ok(v) => v, Err(_) => break };
        let link = if popular {
            card.select(b"div > a").ok()
        } else {
            card.select(b".list_r h2 a").ok()
        };
        let link = match link { Some(v) => v, None => continue };
        let mut href = [0u8; 1024];
        let h = link.attr(b"href", &mut href).unwrap_or(b"");
        let slug = match slug_from_url(h) { Some(v) => v, None => continue };
        let mut title = [0u8; 2048];
        let t = if popular {
            link.attr(b"title", &mut title).ok()
        } else {
            link.attr(b"title", &mut title).ok().filter(|v| !v.is_empty())
                .or_else(|| link.text(&mut title).ok())
        }.unwrap_or(b"");
        if t.is_empty() { continue; }
        let mut image = [0u8; 2048];
        let img = card.select(if popular { b"img" } else { b"img.thumb" }).ok();
        let raw_cover = img.as_ref().and_then(|v| {
            let x = v.attr(b"data-src", &mut image).ok().unwrap_or(b"");
            if x.is_empty() { v.attr(b"src", &mut image).ok() } else { Some(x) }
        }).unwrap_or(b"");
        let mut cover = [0u8; 4096];
        let cv = absolute_url(raw_cover, &mut cover);
        if count > 0 && !write_bytes(p, &mut c, b",") {
            return write_error(operation, "internal_error", "payload overflow");
        }
        if !write_item(p, &mut c, slug, t, cv) {
            return write_error(operation, "internal_error", "payload overflow");
        }
        count += 1;
    }
    let has_more = !popular && count > 0 && doc.select(b".pager a[rel=next]").is_ok();
    let ok = write_bytes(p, &mut c, br#"],"page":{"nextCursor":"#)
        && if has_more {
            write_bytes(p, &mut c, b"\"") && write_usize(p, &mut c, page + 1) && write_bytes(p, &mut c, b"\"")
        } else { write_bytes(p, &mut c, b"null") }
        && write_bytes(p, &mut c, if has_more { br#","hasMore":true}}"# } else { br#","hasMore":false}}"# });
    if !ok { return write_error(operation, "internal_error", "payload overflow"); }
    write_success_payload(operation, c)
}

fn quoted_value<'a>(src: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let at = find_subslice(src, key)?;
    let mut i = at + key.len();
    while i < src.len() && src[i].is_ascii_whitespace() { i += 1; }
    if src.get(i) == Some(&b':') { i += 1; }
    while i < src.len() && src[i].is_ascii_whitespace() { i += 1; }
    if src.get(i) != Some(&b'"') { return None; }
    i += 1;
    let start = i;
    while i < src.len() {
        if src[i] == b'\\' { i = i.saturating_add(2); continue; }
        if src[i] == b'"' { return Some(&src[start..i]); }
        i += 1;
    }
    None
}

fn run_search(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"query") {
        Some(v) => v, None => return write_error("search", "invalid_request", "missing query"),
    };
    let mut query = [0u8; 2048];
    let qn = match decode(raw, &mut query) {
        Ok(n) if n > 0 => n, _ => return write_error("search", "invalid_request", "empty query"),
    };
    let page = request_page_number(req).max(1);
    let mut search_page = [0u8; 4096];
    let mut sp = 0usize;
    if !write_bytes(&mut search_page, &mut sp, BASE)
        || !write_bytes(&mut search_page, &mut sp, b"/searchcse?q=")
        || !write_url_encoded(&mut search_page, &mut sp, &query[..qn]) {
        return write_error("search", "internal_error", "search URL overflow");
    }
    let mut js_url = [0u8; 512];
    let mut jn = 0usize;
    if !write_bytes(&mut js_url, &mut jn, b"https://cse.google.com/cse.js?cx=")
        || !write_bytes(&mut js_url, &mut jn, CSE_CX) {
        return write_error("search", "internal_error", "CSE URL overflow");
    }
    let js_len = match get(&js_url[..jn], Some(&search_page[..sp])) {
        Ok(n) => n, Err(_) => return write_error("search", "network_error", "CSE config request failed"),
    };
    let script = &body_buf()[..js_len];
    let token = match quoted_value(script, b"\"cse_token\"") {
        Some(v) => v, None => return write_error("search", "parse_error", "CSE token missing"),
    };
    let lib = match quoted_value(script, b"\"cselibVersion\"") {
        Some(v) => v, None => return write_error("search", "parse_error", "CSE library version missing"),
    };
    let exp_raw = match find_subslice(script, b"\"exp\"") {
        Some(v) => &script[v..], None => return write_error("search", "parse_error", "CSE experiment config missing"),
    };
    let exp_start = match find_subslice(exp_raw, b"[") { Some(v) => v + 1, None => return write_error("search", "parse_error", "CSE exp array missing") };
    let exp_end = match find_subslice(&exp_raw[exp_start..], b"]") { Some(v) => exp_start + v, None => return write_error("search", "parse_error", "CSE exp array invalid") };
    let mut exp = [0u8; 2048];
    let mut ec = 0usize;
    let mut i = exp_start;
    while i < exp_end {
        if exp_raw[i] == b'"' {
            i += 1;
            let start = i;
            while i < exp_end && exp_raw[i] != b'"' { i += 1; }
            if i > start {
                if ec > 0 { exp[ec] = b','; ec += 1; }
                if ec + i - start > exp.len() { return write_error("search", "internal_error", "CSE exp overflow"); }
                exp[ec..ec + i - start].copy_from_slice(&exp_raw[start..i]);
                ec += i - start;
            }
        }
        i += 1;
    }
    let start = (page - 1).saturating_mul(20);
    let mut url = [0u8; 8192];
    let mut c = 0usize;
    let ok = write_bytes(&mut url, &mut c, b"https://cse.google.com/cse/element/v1?rsz=filtered_cse&num=20&hl=zh-TW&source=gcsc&start=")
        && write_usize(&mut url, &mut c, start)
        && write_bytes(&mut url, &mut c, b"&cselibv=") && write_url_encoded(&mut url, &mut c, lib)
        && write_bytes(&mut url, &mut c, b"&cx=") && write_url_encoded(&mut url, &mut c, CSE_CX)
        && write_bytes(&mut url, &mut c, b"&q=") && write_url_encoded(&mut url, &mut c, &query[..qn])
        && write_bytes(&mut url, &mut c, b"&safe=off&cse_tok=") && write_url_encoded(&mut url, &mut c, token)
        && write_bytes(&mut url, &mut c, b"&sort=&exp=") && write_url_encoded(&mut url, &mut c, &exp[..ec])
        && write_bytes(&mut url, &mut c, b"&callback=google.search.cse.api1&rurl=") && write_url_encoded(&mut url, &mut c, &search_page[..sp]);
    if !ok { return write_error("search", "internal_error", "CSE request URL overflow"); }
    let response_len = match get(&url[..c], Some(&search_page[..sp])) {
        Ok(n) => n, Err(_) => return write_error("search", "network_error", "Google CSE search failed"),
    };
    let response = &body_buf()[..response_len];
    let obj_start = match find_subslice(response, b"{") { Some(v) => v, None => return write_error("search", "parse_error", "invalid CSE response") };
    let obj_end = match response.iter().rposition(|b| *b == b'}') { Some(v) if v > obj_start => v + 1, _ => return write_error("search", "parse_error", "invalid CSE JSONP") };
    let object = &response[obj_start..obj_end];
    let results = match json_object_member(object, b"results") {
        Some(v) => v, None => return write_error("search", "parse_error", "CSE results missing"),
    };
    let mut iter = match JsonArrayIter::from_raw_array(results) {
        Some(v) => v, None => return write_error("search", "parse_error", "CSE results invalid"),
    };
    let p = payload_buf();
    let mut out = 0usize;
    if !write_bytes(p, &mut out, br#"{"items":["#) { return write_error("search", "internal_error", "payload overflow"); }
    let mut count = 0usize;
    while let Some(item) = iter.next_object() {
        let raw_url = extract_json_string(item, b"unescapedUrl").unwrap_or(b"");
        let mut decoded_url = [0u8; 2048];
        let ul = match decode(raw_url, &mut decoded_url) { Ok(v) => v, Err(_) => continue };
        let slug = match slug_from_url(&decoded_url[..ul]) { Some(v) => v, None => continue };
        let rich = match json_object_member(item, b"richSnippet") { Some(v) => v, None => continue };
        let meta = match json_object_member(rich, b"metatags") { Some(v) => v, None => continue };
        let raw_title = extract_json_string(meta, b"ogTitle").unwrap_or(b"");
        let mut title_buf = [0u8; 1024];
        let tl = match decode(raw_title, &mut title_buf) { Ok(v) => v, Err(_) => continue };
        let title = title_buf[..tl].strip_prefix("《".as_bytes()).unwrap_or(&title_buf[..tl]);
        let title = title.split(|b| *b == "》".as_bytes()[0]).next().unwrap_or(title);
        let raw_cover = extract_json_string(meta, b"ogImage").unwrap_or(b"");
        let mut cover_buf = [0u8; 2048];
        let cl = decode(raw_cover, &mut cover_buf).unwrap_or(0);
        if title.is_empty() { continue; }
        if count > 0 && !write_bytes(p, &mut out, b",") { return write_error("search", "internal_error", "payload overflow"); }
        if !write_item(p, &mut out, slug, title, &cover_buf[..cl]) { return write_error("search", "internal_error", "payload overflow"); }
        count += 1;
    }
    let has_more = count >= 20;
    let ok = write_bytes(p, &mut out, br#"],"page":{"nextCursor":"#)
        && if has_more { write_bytes(p, &mut out, b"\"") && write_usize(p, &mut out, page + 1) && write_bytes(p, &mut out, b"\"") } else { write_bytes(p, &mut out, b"null") }
        && write_bytes(p, &mut out, if has_more { br#","hasMore":true}}"# } else { br#","hasMore":false}}"# });
    if !ok { return write_error("search", "internal_error", "payload overflow"); }
    write_success_payload("search", out)
}

fn run_get_manga(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"mangaId") { Some(v) => v, None => return write_error("get_manga", "invalid_request", "missing mangaId") };
    let mut id = [0u8; 512];
    let n = match decode(raw, &mut id) { Ok(v) => v, Err(_) => return write_error("get_manga", "invalid_request", "invalid mangaId") };
    let slug = id[..n].strip_prefix(b"manga:").unwrap_or(&id[..n]);
    let mut url = [0u8; 1024];
    let mut c = 0usize;
    if !write_bytes(&mut url, &mut c, BASE) || !write_bytes(&mut url, &mut c, b"/manhua-")
        || !write_bytes(&mut url, &mut c, slug) || !write_bytes(&mut url, &mut c, b".htm") {
        return write_error("get_manga", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..c], Some(BASE)) { Ok(v) => v, Err(_) => return write_error("get_manga", "network_error", "detail request failed") };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) { Ok(v) => v, Err(_) => return write_error("get_manga", "parse_error", "invalid detail HTML") };
    let mut title = [0u8; 2048];
    let mut cover = [0u8; 2048];
    let mut desc = [0u8; 8192];
    let t = doc.select(b".list_navbox h3 a[href*='/manhua-']").ok().and_then(|v| v.text(&mut title).ok()).unwrap_or(b"");
    let cv = doc.select(b".gameshows img").ok().and_then(|v| v.attr(b"src", &mut cover).ok()).unwrap_or(b"");
    let d = doc.select(b".gameshows dd:last-of-type").ok().and_then(|v| v.text(&mut desc).ok()).unwrap_or(b"");
    if t.is_empty() { return write_error("get_manga", "parse_error", "manga title missing"); }
    let mut abs = [0u8; 4096];
    let cover_url = absolute_url(cv, &mut abs);
    let p = payload_buf();
    let mut o = 0usize;
    let ok = write_bytes(p, &mut o, br#"{"manga":{"id":"manga:"#) && append_json_escaped(p, &mut o, slug)
        && write_bytes(p, &mut o, br#"","title":""#) && append_json_escaped(p, &mut o, t)
        && write_bytes(p, &mut o, br#"","description":""#) && append_json_escaped(p, &mut o, d)
        && write_bytes(p, &mut o, br#"","cover":{"kind":"url","url":""#) && append_json_escaped(p, &mut o, cover_url)
        && write_bytes(p, &mut o, br#""},"authors":[],"artists":[],"status":"unknown","contentRating":"unknown","language":"zh","tags":[],"links":[{"kind":"source","url":""#)
        && append_json_escaped(p, &mut o, &url[..c]) && write_bytes(p, &mut o, br#""}]}}"#);
    if !ok { return write_error("get_manga", "internal_error", "payload overflow"); }
    write_success_payload("get_manga", o)
}

fn run_get_chapters(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"mangaId") { Some(v) => v, None => return write_error("get_chapters", "invalid_request", "missing mangaId") };
    let mut id = [0u8; 512];
    let n = match decode(raw, &mut id) { Ok(v) => v, Err(_) => return write_error("get_chapters", "invalid_request", "invalid mangaId") };
    let slug = id[..n].strip_prefix(b"manga:").unwrap_or(&id[..n]);
    let mut url = [0u8; 1024];
    let mut c = 0usize;
    if !write_bytes(&mut url, &mut c, BASE) || !write_bytes(&mut url, &mut c, b"/manhua-")
        || !write_bytes(&mut url, &mut c, slug) || !write_bytes(&mut url, &mut c, b".htm") {
        return write_error("get_chapters", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..c], Some(BASE)) { Ok(v) => v, Err(_) => return write_error("get_chapters", "network_error", "detail request failed") };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) { Ok(v) => v, Err(_) => return write_error("get_chapters", "parse_error", "invalid detail HTML") };
    let p = payload_buf();
    let mut o = 0usize;
    if !write_bytes(p, &mut o, br#"{"items":["#) { return write_error("get_chapters", "internal_error", "payload overflow"); }
    let mut count = 0usize;
    for i in 1..=2000 {
        let mut selector = [0u8; 128];
        let sel = match nth(b"#comic_chapter li", i, &mut selector) { Ok(v) => v, Err(_) => break };
        let li = match doc.select(sel) { Ok(v) => v, Err(_) => break };
        let a = match li.select(b"a") { Ok(v) => v, Err(_) => continue };
        let mut href = [0u8; 512];
        let h = a.attr(b"href", &mut href).unwrap_or(b"");
        let cid = match chapter_from_url(h) { Some(v) => v, None => continue };
        let mut title = [0u8; 1024];
        let t = a.text(&mut title).unwrap_or(b"");
        if t.is_empty() { continue; }
        if count > 0 && !write_bytes(p, &mut o, b",") { return write_error("get_chapters", "internal_error", "payload overflow"); }
        let ok = write_bytes(p, &mut o, br#"{"id":"chapter:"#) && append_json_escaped(p, &mut o, slug)
            && write_bytes(p, &mut o, b":") && append_json_escaped(p, &mut o, cid)
            && write_bytes(p, &mut o, br#"","mangaId":"manga:"#) && append_json_escaped(p, &mut o, slug)
            && write_bytes(p, &mut o, br#"","title":""#) && append_json_escaped(p, &mut o, t)
            && write_bytes(p, &mut o, br#"","chapterNumber":null}"#);
        if !ok { return write_error("get_chapters", "internal_error", "payload overflow"); }
        count += 1;
    }
    if count == 0 { return write_error("get_chapters", "parse_error", "chapter list empty or not recognized"); }
    if !write_bytes(p, &mut o, br#"],"page":{"nextCursor":null,"hasMore":false}}"#) { return write_error("get_chapters", "internal_error", "payload overflow"); }
    write_success_payload("get_chapters", o)
}

fn run_get_pages(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req, b"chapterId") { Some(v) => v, None => return write_error("get_pages", "invalid_request", "missing chapterId") };
    let mut id = [0u8; 1024];
    let n = match decode(raw, &mut id) { Ok(v) => v, Err(_) => return write_error("get_pages", "invalid_request", "invalid chapterId") };
    let tail = match id[..n].strip_prefix(b"chapter:") { Some(v) => v, None => return write_error("get_pages", "invalid_request", "invalid chapter id") };
    let sep = match find_subslice(tail, b":") { Some(v) => v, None => return write_error("get_pages", "invalid_request", "invalid chapter id") };
    let cid = &tail[sep + 1..];
    let mut url = [0u8; 1024];
    let mut c = 0usize;
    if !write_bytes(&mut url, &mut c, BASE) || !write_bytes(&mut url, &mut c, b"/view-")
        || !write_bytes(&mut url, &mut c, cid) || !write_bytes(&mut url, &mut c, b".htm") {
        return write_error("get_pages", "internal_error", "URL overflow");
    }
    let hn = match get(&url[..c], Some(BASE)) { Ok(v) => v, Err(_) => return write_error("get_pages", "network_error", "reader page request failed") };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) { Ok(v) => v, Err(_) => return write_error("get_pages", "parse_error", "invalid reader HTML") };
    let p = payload_buf();
    let mut o = 0usize;
    if !write_bytes(p, &mut o, br#"{"pages":["#) { return write_error("get_pages", "internal_error", "payload overflow"); }
    let mut count = 0usize;
    for i in 1..=500 {
        let mut selector = [0u8; 160];
        let sel = match nth(b"#pic_list .pic[_src]", i, &mut selector) { Ok(v) => v, Err(_) => break };
        let img = match doc.select(sel) { Ok(v) => v, Err(_) => break };
        let mut raw_url = [0u8; 4096];
        let u = img.attr(b"_src", &mut raw_url).unwrap_or(b"");
        if u.is_empty() { continue; }
        let mut abs = [0u8; 4096];
        let image_url = absolute_url(u, &mut abs);
        if count > 0 && !write_bytes(p, &mut o, b",") { return write_error("get_pages", "internal_error", "payload overflow"); }
        let ok = write_bytes(p, &mut o, br#"{"id":"page:"#) && write_usize(p, &mut o, count)
            && write_bytes(p, &mut o, br#"","index":"#) && write_usize(p, &mut o, count)
            && write_bytes(p, &mut o, br#","image":{"kind":"url","url":""#) && append_json_escaped(p, &mut o, image_url)
            && write_bytes(p, &mut o, br#""}}"#);
        if !ok { return write_error("get_pages", "internal_error", "payload overflow"); }
        count += 1;
    }
    if count == 0 { return write_error("get_pages", "parse_error", "reader images not found"); }
    if !write_bytes(p, &mut o, br#"],"page":{"nextCursor":null,"hasMore":false}}"#) { return write_error("get_pages", "internal_error", "payload overflow"); }
    write_success_payload("get_pages", o)
}

fn run_get_listings(_req: &[u8]) -> u32 {
    let data = br#"{"listings":[{"id":"popular","name":"热门推荐"},{"id":"cate-5","name":"爱情"},{"id":"cate-4","name":"科幻"},{"id":"cate-10","name":"校园"},{"id":"cate-33","name":"高清单行本"},{"id":"letter-a","name":"A字母"},{"id":"letter-d","name":"D字母"}]}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_listings", data.len())
}

fn run_get_manga_list(req: &[u8]) -> u32 {
    let args = json_object_member(req, b"args").unwrap_or(req);
    let listing = extract_json_string(args, b"listingId").unwrap_or(b"popular");
    let mut id = [0u8; 128];
    let n = match decode(listing, &mut id) { Ok(v) => v, Err(_) => return write_error("get_manga_list", "invalid_request", "invalid listing id") };
    let path = if &id[..n] == b"popular" { b"/" as &[u8] }
        else if id[..n].starts_with(b"cate-") {
            let mut p = [0u8; 128]; let mut c = 0usize;
            if !write_bytes(&mut p, &mut c, b"/cate-") || !write_bytes(&mut p, &mut c, &id[5..n]) || !write_bytes(&mut p, &mut c, b".htm") {
                return write_error("get_manga_list", "internal_error", "path overflow");
            }
            return fetch_listing_path(&p[..c], id[..n] == *b"popular", request_page_number(req));
        } else if id[..n].starts_with(b"letter-") {
            let mut p = [0u8; 128]; let mut c = 0usize;
            if !write_bytes(&mut p, &mut c, b"/pinyin-") || !write_bytes(&mut p, &mut c, &id[7..n]) || !write_bytes(&mut p, &mut c, b".htm") {
                return write_error("get_manga_list", "internal_error", "path overflow");
            }
            return fetch_listing_path(&p[..c], false, request_page_number(req));
        } else { b"/" };
    fetch_listing_path(path, path == b"/", request_page_number(req))
}

fn fetch_listing_path(path: &[u8], popular: bool, page: usize) -> u32 {
    let mut url = [0u8; 1024];
    let mut c = 0usize;
    if !write_bytes(&mut url, &mut c, BASE) || !write_bytes(&mut url, &mut c, path) {
        return write_error("get_manga_list", "internal_error", "URL overflow");
    }
    if !popular {
        if !write_bytes(&mut url, &mut c, if find_subslice(path, b"?").is_some() { b"&page=" } else { b"?page=" })
            || !write_usize(&mut url, &mut c, page) {
            return write_error("get_manga_list", "internal_error", "URL overflow");
        }
    }
    let hn = match get(&url[..c], Some(BASE)) { Ok(v) => v, Err(_) => return write_error("get_manga_list", "network_error", "listing request failed") };
    let doc = match HtmlNode::parse(&body_buf()[..hn]) { Ok(v) => v, Err(_) => return write_error("get_manga_list", "parse_error", "invalid listing HTML") };
    parse_cards(&doc, "get_manga_list", popular, page)
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
    let raw = match extract_json_string(req, b"url") { Some(v) => v, None => return write_error("get_image_request", "invalid_request", "missing URL") };
    let p = payload_buf();
    let mut c = 0usize;
    let ok = write_bytes(p, &mut c, br#"{"url":""#) && append_json_escaped(p, &mut c, raw)
        && write_bytes(p, &mut c, br#"","headers":{"Referer":"https://comic.acgn.cc/","User-Agent":"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130 Safari/537.36"}}"#);
    if !ok { return write_error("get_image_request", "internal_error", "payload overflow"); }
    write_success_payload("get_image_request", c)
}

koma_source_sdk::koma_source_exports!("acgn");
