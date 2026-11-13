//! # Example 如漫画
//!
//! A minimal, fully functional Koma source that demonstrates:
//! - Using SDK macros (koma_source_buffers!, koma_source_helpers!, koma_source_exports!)
//! - JSON API source pattern (fetch JSON, parse with extract_json_string / JsonArrayIter)
//! - HTML scraping pattern (html_parse → html_select → html_attr → html_text → html_close)
//! - Building JSON responses manually in no_std
//!
//! This source uses the free JSONPlaceholder API (https://jsonplaceholder.typicode.com)
//! to provide fake manga data. It's meant as a learning reference, not a real source.

#![no_std]

use koma_source_sdk::host;
use koma_source_sdk::json_utils::{
    append_json_escaped, extract_json_string, find_subslice, json_object_member,
    write_bytes, write_url_encoded, write_usize, JsonArrayIter,
};
use koma_source_sdk::source::{SourceCapabilities, SourceInfo};

// ── Source metadata ──────────────────────────────────────────────
// These are displayed in the Koma app source list.

const SOURCE_INFO: SourceInfo = SourceInfo {
    id: "com.rumanhua.koma",
    name: "如漫画",
    language: "zh-Hans",
    version: "0.1.0",
    api_version: "0.2",
    description: "如漫画网站公开漫画源",
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
    settings: false,
    home: false,
    credentials: false,
    image_request: false,
};

// ── Buffers ──────────────────────────────────────────────────────
// These define static memory for the WASM module.
// - payload: where you build the output JSON
// - http_out: where the host writes HTTP response body
// - body: decoded body (after stripping HTTP status line)
// - http_req: where you build the HTTP request (URL + headers)
// - scratch: temporary working buffers (scratch_a, scratch_b)

koma_source_sdk::koma_source_buffers! {
    payload: 64 * 1024,
    http_out: 256 * 1024,
    body: 256 * 1024,
    http_req: 2048,
    scratch: 4096,
}

// ── Helpers ──────────────────────────────────────────────────────
// The macro generates: write_error, write_success_payload, read_request,
// trim_ascii, decode_json_body, fetch_get

koma_source_sdk::koma_source_helpers!();


const SITE: &[u8] = b"http://www.rumanhua2.com";

struct HtmlNode(host::HtmlDescriptor);
impl Drop for HtmlNode { fn drop(&mut self) { let _ = host::html_close(self.0); } }
impl HtmlNode {
    fn parse(html: &[u8]) -> Result<Self, ()> { host::html_parse(html).map(Self).map_err(|_| ()) }
    fn select(&self, selector: &[u8]) -> Result<Self, ()> { host::html_select(self.0, selector).map(Self).map_err(|_| ()) }
    fn attr<'a>(&self, key: &[u8], out: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let n = host::html_attr(self.0, key, out).map_err(|_| ())?;
        if n > out.len() { return Err(()); } Ok(&out[..n])
    }
    fn text<'a>(&self, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let n = host::html_text(self.0, out).map_err(|_| ())?;
        if n > out.len() { return Err(()); }
        let mut v = &out[..n];
        while v.first().is_some_and(|b| b.is_ascii_whitespace()) { v = &v[1..]; }
        while v.last().is_some_and(|b| b.is_ascii_whitespace()) { v = &v[..v.len()-1]; }
        Ok(v)
    }
}
fn post_form(url: &[u8], form: &[u8]) -> Result<usize, ()> {
    let mut enc = [0u8; 8192];
    let mut n = 0usize;
    const T: &[u8;64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i=0;
    while i<form.len() {
        let a=form[i] as u32; let b=if i+1<form.len(){form[i+1] as u32}else{0};
        let c=if i+2<form.len(){form[i+2] as u32}else{0};
        if n+4>enc.len(){return Err(());}
        enc[n]=T[(a>>2) as usize]; enc[n+1]=T[(((a&3)<<4)|(b>>4)) as usize];
        enc[n+2]=if i+1<form.len(){T[(((b&15)<<2)|(c>>6)) as usize]}else{b'='};
        enc[n+3]=if i+2<form.len(){T[(c&63) as usize]}else{b'='};
        n+=4; i+=3;
    }
    let req = http_req_buf();
    let len = koma_source_sdk::build_post_request(req,url,&enc[..n],b"application/x-www-form-urlencoded",Some(SITE)).ok_or(())?;
    let got = host::http_request(&req[..len],http_out()).map_err(|_|())?;
    koma_source_sdk::decode_json_body_into(&http_out()[..got],body_buf()).map_err(|_|())
}
fn nth<'a>(base: &[u8], index: usize, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
    let mut n=0;
    if !write_bytes(out,&mut n,base) || !write_bytes(out,&mut n,b":nth-of-type(")
        || !write_usize(out,&mut n,index) || !write_bytes(out,&mut n,b")") { return Err(()); }
    Ok(&out[..n])
}
fn absolute_url<'a>(raw: &[u8], out: &'a mut [u8]) -> &'a [u8] {
    let mut n=0;
    if raw.starts_with(b"http://") || raw.starts_with(b"https://") { let _=write_bytes(out,&mut n,raw); }
    else if raw.starts_with(b"//") { let _=write_bytes(out,&mut n,b"http:"); let _=write_bytes(out,&mut n,raw); }
    else { let _=write_bytes(out,&mut n,SITE); if !raw.starts_with(b"/") { let _=write_bytes(out,&mut n,b"/"); } let _=write_bytes(out,&mut n,raw); }
    &out[..n]
}
fn json_decode(raw: &[u8], out: &mut [u8]) -> Result<usize, ()> {
    koma_source_sdk::plain_text::decode_json_content(raw,out).map(|v|v.len()).map_err(|_|())
}

// ── Search ───────────────────────────────────────────────────────
// Input:  {"query":"one piece","page":1,"limit":25}
// Output: {"items":[...],"page":{"hasMore":false}}
//
// We fake it by fetching posts from JSONPlaceholder and wrapping them
// as "manga" results.


fn run_search(req: &[u8]) -> u32 {
    let raw = match extract_json_string(req,b"query") { Some(v)=>v, None=>return write_error("search","invalid_request","missing query") };
    let mut query=[0u8;2048];
    let qn=match json_decode(raw,&mut query) { Ok(n) if n>0=>n, _=>return write_error("search","invalid_request","empty query") };
    let mut form=[0u8;4096]; let mut fnn=0;
    if !write_bytes(&mut form,&mut fnn,b"k=") || !write_url_encoded(&mut form,&mut fnn,&query[..qn]) {
        return write_error("search","internal_error","form overflow");
    }
    let mut url=[0u8;256]; let mut un=0;
    if !write_bytes(&mut url,&mut un,SITE) || !write_bytes(&mut url,&mut un,b"/s") {
        return write_error("search","internal_error","URL overflow");
    }
    let hn=match post_form(&url[..un],&form[..fnn]) { Ok(n)=>n, Err(_)=>return write_error("search","network_error","search request failed") };
    let doc=match HtmlNode::parse(&body_buf()[..hn]) { Ok(v)=>v, Err(_)=>return write_error("search","parse_error","invalid search HTML") };
    let p=payload_buf(); let mut c=0;
    if !write_bytes(p,&mut c,br#"{"items":["#) { return write_error("search","internal_error","overflow"); }
    let mut count=0;
    for i in 1..=30 {
        let mut selector=[0u8;128]; let sel=match nth(b".col-auto",i,&mut selector) { Ok(v)=>v,Err(_)=>break };
        let card=match doc.select(sel) { Ok(v)=>v,Err(_)=>continue };
        let link=match card.select(b"a") { Ok(v)=>v,Err(_)=>continue };
        let mut href=[0u8;512]; let h=link.attr(b"href",&mut href).unwrap_or(b"");
        let slug=h.strip_prefix(b"/").unwrap_or(h).split(|b|*b==b'/').next().unwrap_or(b"");
        if slug.is_empty(){continue;}
        let mut title=[0u8;1024]; let mut author=[0u8;512]; let mut img=[0u8;2048];
        let title_node=card.select(b".e-title").ok();
        let title_text=title_node.as_ref().and_then(|v|v.text(&mut title).ok()).unwrap_or(b"");
        let author_node=card.select(b".tip").ok();
        let author_text=author_node.as_ref().and_then(|v|v.text(&mut author).ok()).unwrap_or(b"");
        let image=card.select(b"img").ok();
        let img_raw=if let Some(ref v)=image { let x=v.attr(b"data-src",&mut img).unwrap_or(b""); if x.is_empty(){v.attr(b"src",&mut img).unwrap_or(b"")}else{x} } else { b"" };
        let mut cover=[0u8;4096]; let cover_url=absolute_url(img_raw,&mut cover);
        let title_text=if title_text.is_empty() { image.as_ref().and_then(|v|v.attr(b"alt",&mut title).ok()).unwrap_or(b"") } else { title_text };
        if title_text.is_empty(){continue;}
        if count>0 && !write_bytes(p,&mut c,b","){return write_error("search","internal_error","overflow");}
        let ok=write_bytes(p,&mut c,br#"{"id":"manga:"#) && append_json_escaped(p,&mut c,slug)
            && write_bytes(p,&mut c,br#"","title":""#) && append_json_escaped(p,&mut c,title_text)
            && write_bytes(p,&mut c,br#"","cover":{"kind":"url","url":""#) && append_json_escaped(p,&mut c,cover_url)
            && write_bytes(p,&mut c,br#""},"authors":[""#) && append_json_escaped(p,&mut c,author_text)
            && write_bytes(p,&mut c,br#""],"status":"unknown","contentRating":"unknown"}"#);
        if !ok{return write_error("search","internal_error","overflow");} count+=1;
    }
    if !write_bytes(p,&mut c,br#"],"page":{"nextCursor":null,"hasMore":false}}"#){return write_error("search","internal_error","overflow");}
    write_success_payload("search",c)
}


// ── Get manga detail ─────────────────────────────────────────────
// Input:  {"mangaId":"demo:1"}
// Output: {"manga":{"id":"...","title":"...","description":"...",...}}


fn run_get_manga(req: &[u8]) -> u32 {
    let raw=match extract_json_string(req,b"mangaId"){Some(v)=>v,None=>return write_error("get_manga","invalid_request","missing mangaId")};
    let mut id=[0u8;256]; let n=match json_decode(raw,&mut id){Ok(n)=>n,Err(_)=>return write_error("get_manga","invalid_request","bad mangaId")};
    let manga=if id[..n].starts_with(b"manga:"){&id[6..n]}else{&id[..n]};
    let mut url=[0u8;512];let mut un=0;
    if !write_bytes(&mut url,&mut un,SITE)||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,manga)||!write_bytes(&mut url,&mut un,b"/"){return write_error("get_manga","internal_error","URL overflow");}
    let hn=match fetch_get(&url[..un],Some(SITE)){Ok(n)=>n,Err(_)=>return write_error("get_manga","network_error","detail request failed")};
    let doc=match HtmlNode::parse(&body_buf()[..hn]){Ok(v)=>v,Err(_)=>return write_error("get_manga","parse_error","invalid detail HTML")};
    let mut title=[0u8;2048];let mut desc=[0u8;8192];let mut cover=[0u8;2048];
    let t=doc.select(b"h1.name_mh").ok().and_then(|v|v.text(&mut title).ok()).unwrap_or(b"");
    let d=doc.select(b"p.content").ok().and_then(|v|v.text(&mut desc).ok()).unwrap_or(b"");
    let raw_cover=doc.select(b"meta[property='og:image']").ok().and_then(|v|v.attr(b"content",&mut cover).ok()).unwrap_or(b"");
    if t.is_empty(){return write_error("get_manga","parse_error","title not found");}
    let mut abs=[0u8;4096];let cv=absolute_url(raw_cover,&mut abs);
    let p=payload_buf();let mut c=0;
    let ok=write_bytes(p,&mut c,br#"{"manga":{"id":"manga:"#)&&append_json_escaped(p,&mut c,manga)
      &&write_bytes(p,&mut c,br#"","title":""#)&&append_json_escaped(p,&mut c,t)
      &&write_bytes(p,&mut c,br#"","description":""#)&&append_json_escaped(p,&mut c,d)
      &&write_bytes(p,&mut c,br#"","cover":{"kind":"url","url":""#)&&append_json_escaped(p,&mut c,cv)
      &&write_bytes(p,&mut c,br#""},"authors":[],"status":"unknown","language":"zh-Hans","contentRating":"unknown","tags":[],"links":[]}}"#);
    if !ok{return write_error("get_manga","internal_error","overflow");}write_success_payload("get_manga",c)
}


// ── Get chapters ─────────────────────────────────────────────────
// Input:  {"mangaId":"demo:1"}
// Output: {"items":[{"id":"ch:demo:1:1","title":"Chapter 1",...}],...}
//
// We generate 3 fake chapters for any manga.



static mut CHAPTER_IDS: [[u8; 256]; 1024] = [[0; 256]; 1024];
static mut CHAPTER_NAMES: [[u8; 1024]; 1024] = [[0; 1024]; 1024];
static mut CHAPTER_ID_LENS: [usize; 1024] = [0; 1024];
static mut CHAPTER_NAME_LENS: [usize; 1024] = [0; 1024];
static mut CHAPTER_COUNT: usize = 0;

fn remember_chapter(id: &[u8], name: &[u8]) {
    if id.is_empty() || name.is_empty() || id.len() >= 256 || name.len() >= 1024 { return; }
    unsafe {
        for i in 0..CHAPTER_COUNT {
            if &CHAPTER_IDS[i][..CHAPTER_ID_LENS[i]] == id { return; }
        }
        if CHAPTER_COUNT < 1024 {
            let i = CHAPTER_COUNT;
            CHAPTER_IDS[i][..id.len()].copy_from_slice(id);
            CHAPTER_NAMES[i][..name.len()].copy_from_slice(name);
            CHAPTER_ID_LENS[i] = id.len();
            CHAPTER_NAME_LENS[i] = name.len();
            CHAPTER_COUNT += 1;
        }
    }
}


fn run_get_chapters(req: &[u8]) -> u32 {
    let raw=match extract_json_string(req,b"mangaId"){Some(v)=>v,None=>return write_error("get_chapters","invalid_request","missing mangaId")};
    let mut id=[0u8;256];let n=match json_decode(raw,&mut id){Ok(n)=>n,Err(_)=>return write_error("get_chapters","invalid_request","bad mangaId")};
    let manga=if id[..n].starts_with(b"manga:"){&id[6..n]}else{&id[..n]};
    unsafe { CHAPTER_COUNT=0; }
    let mut url=[0u8;512];let mut un=0;
    if !write_bytes(&mut url,&mut un,SITE)||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,manga)||!write_bytes(&mut url,&mut un,b"/"){return write_error("get_chapters","internal_error","URL overflow");}
    let hn=match fetch_get(&url[..un],Some(SITE)){Ok(n)=>n,Err(_)=>return write_error("get_chapters","network_error","detail request failed")};
    let doc=match HtmlNode::parse(&body_buf()[..hn]){Ok(v)=>v,Err(_)=>return write_error("get_chapters","parse_error","invalid detail HTML")};
    for i in 1..=512 {
        let mut selector=[0u8;128];let sel=match nth(b".chapterlistload a",i,&mut selector){Ok(v)=>v,Err(_)=>break};
        let a=match doc.select(sel){Ok(v)=>v,Err(_)=>break};
        let mut href=[0u8;1024];let h=a.attr(b"href",&mut href).unwrap_or(b"");
        let tail=h.rsplit(|b|*b==b'/').next().unwrap_or(b"");
        let cid=tail.strip_suffix(b".html").unwrap_or(b"");
        if cid.is_empty(){continue;}
        let mut name=[0u8;1024];
        let title=if let Ok(li)=a.select(b"li"){li.text(&mut name).unwrap_or(b"")}else{a.text(&mut name).unwrap_or(b"")};
        remember_chapter(cid,title);
    }
    let mut form=[0u8;512];let mut fc=0;
    if !write_bytes(&mut form,&mut fc,b"id=")||!write_url_encoded(&mut form,&mut fc,manga){return write_error("get_chapters","internal_error","form overflow");}
    let mut more=[0u8;512];let mut mc=0;
    if write_bytes(&mut more,&mut mc,SITE)&&write_bytes(&mut more,&mut mc,b"/morechapter") {
        if let Ok(jn)=post_form(&more[..mc],&form[..fc]) {
            let response=&body_buf()[..jn];
            if let Some(data)=json_object_member(response,b"data") {
                if let Some(mut iter)=JsonArrayIter::from_raw_array(data) {
                    while let Some(obj)=iter.next_object() {
                        let rid=extract_json_string(obj,b"chapterid").unwrap_or(b"");
                        let rname=extract_json_string(obj,b"chaptername").unwrap_or(b"");
                        let mut cid=[0u8;256];let mut title=[0u8;1024];
                        if let (Ok(il),Ok(tl))=(json_decode(rid,&mut cid),json_decode(rname,&mut title)) {
                            remember_chapter(&cid[..il],&title[..tl]);
                        }
                    }
                }
            }
        }
    }
    let count=unsafe{CHAPTER_COUNT};
    if count==0{return write_error("get_chapters","parse_error","no chapters found");}
    let p=payload_buf();let mut c=0;
    if !write_bytes(p,&mut c,br#"{"items":["#){return write_error("get_chapters","internal_error","overflow");}
    for i in 0..count {
        let idx=count-1-i;
        let (cid,clen,title,tlen)=unsafe{(&CHAPTER_IDS[idx],CHAPTER_ID_LENS[idx],&CHAPTER_NAMES[idx],CHAPTER_NAME_LENS[idx])};
        if i>0&&!write_bytes(p,&mut c,b","){return write_error("get_chapters","internal_error","overflow");}
        let ok=write_bytes(p,&mut c,br#"{"id":"chapter:"#)&&append_json_escaped(p,&mut c,manga)
          &&write_bytes(p,&mut c,b":")&&append_json_escaped(p,&mut c,&cid[..clen])
          &&write_bytes(p,&mut c,br#"","mangaId":"manga:"#)&&append_json_escaped(p,&mut c,manga)
          &&write_bytes(p,&mut c,br#"","title":""#)&&append_json_escaped(p,&mut c,&title[..tlen])
          &&write_bytes(p,&mut c,br#"","language":"zh-Hans"}"#);
        if !ok{return write_error("get_chapters","internal_error","overflow");}
    }
    if !write_bytes(p,&mut c,br#"],"page":{"nextCursor":null,"hasMore":false}}"#){return write_error("get_chapters","internal_error","overflow");}
    write_success_payload("get_chapters",c)
}

// ── Get pages ────────────────────────────────────────────────────
// Input:  {"chapterId":"ch:demo:1:1"}
// Output: {"pages":[{"id":"page:0","index":0,"image":{"kind":"url","url":"..."}}]}


fn run_get_pages(req: &[u8]) -> u32 {
    let raw=match extract_json_string(req,b"chapterId"){Some(v)=>v,None=>return write_error("get_pages","invalid_request","missing chapterId")};
    let mut id=[0u8;512];let n=match json_decode(raw,&mut id){Ok(n)=>n,Err(_)=>return write_error("get_pages","invalid_request","bad chapterId")};
    let v=&id[..n];let tail=match v.strip_prefix(b"chapter:"){Some(v)=>v,None=>return write_error("get_pages","invalid_request","bad chapterId")};
    let sep=match find_subslice(tail,b":"){Some(v)=>v,None=>return write_error("get_pages","invalid_request","bad chapterId")};
    let manga=&tail[..sep];let chapter=&tail[sep+1..];
    let mut url=[0u8;1024];let mut un=0;
    if !write_bytes(&mut url,&mut un,SITE)||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,manga)||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,chapter)||!write_bytes(&mut url,&mut un,b".html"){return write_error("get_pages","internal_error","URL overflow");}
    let _hn=match fetch_get(&url[..un],Some(SITE)){Ok(n)=>n,Err(_)=>return write_error("get_pages","network_error","chapter page request failed")};
    write_error("get_pages","not_implemented","reader image decryption is being integrated")
}


// Optional operations — return not_implemented

fn run_get_listings(_req: &[u8]) -> u32 {
    write_error("get_listings", "not_implemented", "not supported")
}

fn run_get_manga_list(_req: &[u8]) -> u32 {
    write_error("get_manga_list", "not_implemented", "not supported")
}

fn run_get_home(_req: &[u8]) -> u32 {
    write_error("get_home", "not_implemented", "not supported")
}

fn run_get_filters(_req: &[u8]) -> u32 {
    write_error("get_filters", "not_implemented", "not supported")
}

fn run_get_settings(_req: &[u8]) -> u32 {
    write_error("get_settings", "not_implemented", "not supported")
}

fn run_get_image_request(_req: &[u8]) -> u32 {
    write_error("get_image_request", "not_implemented", "not supported")
}

// ── Panic handler (required for no_std) ──────────────────────────
// ── WASM exports ─────────────────────────────────────────────────
// Generates: koma_source_info, koma_source_init, koma_source_search,
// koma_source_get_manga, koma_source_get_chapters, koma_source_get_pages,
// koma_source_get_listings, koma_source_get_manga_list, koma_source_get_home,
// koma_source_get_filters, koma_source_get_settings, koma_source_get_image_request,
// koma_source_free

koma_source_sdk::koma_source_exports!("example-demo");

// ── Utility ──────────────────────────────────────────────────────

fn find_subslice_from(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if haystack.len() < needle.len() {
        return None;
    }
    for i in 0..=haystack.len() - needle.len() {
        if haystack[i..].starts_with(needle) {
            return Some(i);
        }
    }
    None
}
