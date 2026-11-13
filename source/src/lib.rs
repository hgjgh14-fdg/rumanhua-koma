//! # Example 如漫画
//!
//! A minimal, fully functional Koma source that demonstrates:
//! - Using SDK macros (koma_source_buffers!, koma_source_helpers!, koma_source_exports!)
//! - JSON API source pattern (fetch JSON, parse with extract_json_string / JsonArrayIter)
//! - HTML scraping pattern (html_parse → html_select → html_attr → html_text → html_close)
//! - Building JSON responses manually in no_std
//!
//! Source adapter for the public RuManHua comic website.

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
    version: env!("CARGO_PKG_VERSION"),
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
    listings: true,
    manga_list: true,
    filters: false,
    settings: true,
    home: false,
    credentials: false,
    image_request: true,
};

// ── Buffers ──────────────────────────────────────────────────────
// These define static memory for the WASM module.
// - payload: where you build the output JSON
// - http_out: where the host writes HTTP response body
// - body: decoded body (after stripping HTTP status line)
// - http_req: where you build the HTTP request (URL + headers)
// - scratch: temporary working buffers (scratch_a, scratch_b)

koma_source_sdk::koma_source_buffers! {
    payload: 1024 * 1024,
    http_out: 3 * 1024 * 1024,
    body: 3 * 1024 * 1024,
    http_req: 8192,
    scratch: 16384,
}

// ── Helpers ──────────────────────────────────────────────────────
// The macro generates: write_error, write_success_payload, read_request,
// trim_ascii, decode_json_body, fetch_get

koma_source_sdk::koma_source_helpers!();


const SITES: [&[u8]; 1] = [
    b"http://www.rumanhua2.com",
];
static mut ACTIVE_SITE: &[u8] = SITES[0];

fn site() -> &'static [u8] {
    unsafe { ACTIVE_SITE }
}

fn homepage_choices_supported(req: &[u8]) -> bool {
    json_object_member(req, b"hostFeatures")
        .map(|v| {
            let v = v.trim_ascii();
            v.starts_with(b"[")
                && v.ends_with(b"]")
                && v[1..v.len() - 1]
                    .split(|b| *b == b',')
                    .any(|item| item.trim_ascii() == br#""source_homepage_choices_v1""#)
        })
        .unwrap_or(false)
}

// Current Koma hosts use the saved domain choice. Older hosts retain
// automatic mirror probing because they do not expose the setting.
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
        let mut url = [0u8; 256];
        let mut n = 0;
        if !write_bytes(&mut url, &mut n, candidate)
            || !write_bytes(&mut url, &mut n, b"/")
        {
            continue;
        }
        if fetch_get(&url[..n], Some(candidate)).is_ok() {
            unsafe { ACTIVE_SITE = *candidate; }
            return;
        }
    }
    unsafe { ACTIVE_SITE = SITES[0]; }
}
const KEYS: [&[u8]; 10] = [b"smkhy258", b"smkd95fv", b"md496952", b"cdcsdwq", b"vbfsa256", b"cawf151c", b"cd56cvda", b"8kihnt9", b"dso15tlo", b"5ko6plhy"];

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
    let req = http_req_buf();
    let len = koma_source_sdk::build_post_request(
        req, url, form, b"application/x-www-form-urlencoded", Some(site())
    ).ok_or(())?;
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
    else { let _=write_bytes(out,&mut n,site()); if !raw.starts_with(b"/") { let _=write_bytes(out,&mut n,b"/"); } let _=write_bytes(out,&mut n,raw); }
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
    select_site(req);
    let raw = match extract_json_string(req,b"query") { Some(v)=>v, None=>return write_error("search","invalid_request","missing query") };
    let mut query=[0u8;2048];
    let qn=match json_decode(raw,&mut query) { Ok(n) if n>0=>n, _=>return write_error("search","invalid_request","empty query") };
    let mut form=[0u8;4096]; let mut fnn=0;
    if !write_bytes(&mut form,&mut fnn,b"k=") || !write_url_encoded(&mut form,&mut fnn,&query[..qn]) {
        return write_error("search","internal_error","form overflow");
    }
    let mut url=[0u8;256]; let mut un=0;
    if !write_bytes(&mut url,&mut un,site()) || !write_bytes(&mut url,&mut un,b"/s") {
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
        let mut alt=[0u8;1024];
        let alt_text=image.as_ref().and_then(|v|v.attr(b"alt",&mut alt).ok()).unwrap_or(b"");
        let title_text=if title_text.is_empty() { alt_text } else { title_text };
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
    select_site(req);
    let raw=match extract_json_string(req,b"mangaId"){Some(v)=>v,None=>return write_error("get_manga","invalid_request","missing mangaId")};
    let mut id=[0u8;256]; let n=match json_decode(raw,&mut id){Ok(n)=>n,Err(_)=>return write_error("get_manga","invalid_request","bad mangaId")};
    let manga=if id[..n].starts_with(b"manga:"){&id[6..n]}else{&id[..n]};
    let mut url=[0u8;512];let mut un=0;
    if !write_bytes(&mut url,&mut un,site())||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,manga)||!write_bytes(&mut url,&mut un,b"/"){return write_error("get_manga","internal_error","URL overflow");}
    let hn=match fetch_get(&url[..un],Some(site())){Ok(n)=>n,Err(_)=>return write_error("get_manga","network_error","detail request failed")};
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
    select_site(req);
    let raw=match extract_json_string(req,b"mangaId"){Some(v)=>v,None=>return write_error("get_chapters","invalid_request","missing mangaId")};
    let mut id=[0u8;256];let n=match json_decode(raw,&mut id){Ok(n)=>n,Err(_)=>return write_error("get_chapters","invalid_request","bad mangaId")};
    let manga=if id[..n].starts_with(b"manga:"){&id[6..n]}else{&id[..n]};
    unsafe { CHAPTER_COUNT=0; }
    let mut url=[0u8;512];let mut un=0;
    if !write_bytes(&mut url,&mut un,site())||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,manga)||!write_bytes(&mut url,&mut un,b"/"){return write_error("get_chapters","internal_error","URL overflow");}
    let hn=match fetch_get(&url[..un],Some(site())){Ok(n)=>n,Err(_)=>return write_error("get_chapters","network_error","detail request failed")};
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
    if write_bytes(&mut more,&mut mc,site())&&write_bytes(&mut more,&mut mc,b"/morechapter") {
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



static mut PACK_A: [u8; 131072] = [0; 131072];
static mut PACK_B: [u8; 131072] = [0; 131072];
fn word_byte(b:u8)->bool{b.is_ascii_alphanumeric()||b==b'_'}
fn token_for(mut value:usize,radix:usize,out:&mut[u8])->Result<usize,()>{
    if radix<2||radix>62{return Err(());}
    let mut rev=[0u8;32];let mut n=0;
    loop{let r=value%radix;value/=radix;
        rev[n]=if r<10{b'0'+r as u8}else if r<36{b'a'+(r-10)as u8}else{(r as u8)+29};n+=1;
        if value==0{break;}if n>=rev.len(){return Err(());}
    }
    if n>out.len(){return Err(());}
    for i in 0..n{out[i]=rev[n-1-i];}Ok(n)
}
fn quoted_content(raw:&[u8],out:&mut[u8])->Result<usize,()>{
    if raw.len()<2{return Err(());}let quote=raw[0];if !matches!(quote,b'\''|b'"'|b'\x60')||raw[raw.len()-1]!=quote{return Err(());}
    let mut i=1;let mut n=0;
    while i+1<raw.len(){let mut b=raw[i];i+=1;if b==b'\\'{let e=*raw.get(i).ok_or(())?;i+=1;b=match e{b'n'=>b'\n',b'r'=>b'\r',b't'=>b'\t',b'\\'=>b'\\',b'\''=>b'\'',b'"'=>b'"',_=>e};}
        if n>=out.len(){return Err(());}out[n]=b;n+=1;
    }Ok(n)
}
fn extract_cipher(html:&[u8],reader:&mut usize,out:&mut[u8])->Result<usize,()>{
    let rp=find_subslice(html,b"readerContainer").ok_or(())?;
    let tail=&html[rp..];let dp=find_subslice(tail,b"data-id=\"").ok_or(())?+9;
    let digits=tail.get(dp..).ok_or(())?;let dn=digits.iter().position(|b|!b.is_ascii_digit()).ok_or(())?;
    *reader=core::str::from_utf8(&digits[..dn]).map_err(|_|())?.parse::<usize>().map_err(|_|())?;
    let ep=find_subslice(html,b"eval(function(p,a,c,k,e,d)").ok_or(())?;
    let mut pos=ep+4;let mut depth=0i32;let mut quote=0u8;let mut escaped=false;let mut end=0;
    while pos<html.len(){let b=html[pos];if quote!=0{if escaped{escaped=false;}else if b==b'\\'{escaped=true;}else if b==quote{quote=0;}}
        else if matches!(b,b'\''|b'"'|b'\x60'){quote=b;}else if b==b'('{depth+=1;}else if b==b')'{depth-=1;if depth==0{end=pos+1;break;}}pos+=1;}
    if end==0{return Err(());}let expr=&html[ep+4..end];
    let brace=find_subslice(expr,b"{").ok_or(())?;let mut bd=1i32;let mut j=brace+1;quote=0;escaped=false;
    while j<expr.len(){let b=expr[j];if quote!=0{if escaped{escaped=false;}else if b==b'\\'{escaped=true;}else if b==quote{quote=0;}}
        else if matches!(b,b'\''|b'"'|b'\x60'){quote=b;}else if b==b'{'{bd+=1;}else if b==b'}'{bd-=1;if bd==0{break;}}j+=1;}
    if bd!=0{return Err(());}let args=trim_ascii(expr.get(j+1..).ok_or(())?);
    let args=args.strip_prefix(b"(").ok_or(())?.strip_suffix(b")").ok_or(())?;
    let mut ranges=[(0usize,0usize);8];let mut count=0;let mut start=0;let mut d=0i32;quote=0;escaped=false;
    for (i,&b) in args.iter().enumerate(){if quote!=0{if escaped{escaped=false;}else if b==b'\\'{escaped=true;}else if b==quote{quote=0;}}
        else if matches!(b,b'\''|b'"'|b'\x60'){quote=b;}else if matches!(b,b'('|b'['|b'{'){d+=1;}else if matches!(b,b')'|b']'|b'}'){d-=1;}else if b==b','&&d==0{if count<8{ranges[count]=(start,i);count+=1;}start=i+1;}}
    if count<3{return Err(());}if count<8{ranges[count]=(start,args.len());count+=1;}
    let (ps,pe)=ranges[0];let mut pbuf=unsafe{&mut *core::ptr::addr_of_mut!(PACK_A)};
    let pn=quoted_content(trim_ascii(&args[ps..pe]),pbuf)?;
    let (as_,ae)=ranges[1];let radix=core::str::from_utf8(trim_ascii(&args[as_..ae])).map_err(|_|())?.parse::<usize>().map_err(|_|())?;
    let (cs,ce)=ranges[2];let total=core::str::from_utf8(trim_ascii(&args[cs..ce])).map_err(|_|())?.parse::<usize>().map_err(|_|())?;
    let (ks,ke)=ranges[3];let karg=trim_ascii(&args[ks..ke]);let q=karg.first().copied().ok_or(())?;
    let qend=karg[1..].iter().position(|b|*b==q).ok_or(())?+1;let dict_raw=&karg[1..qend];
    let mut offsets=[(0usize,0usize);4096];let mut dc=0;let mut st=0;
    for i in 0..=dict_raw.len(){if i==dict_raw.len()||dict_raw[i]==b'|'{if dc<offsets.len(){offsets[dc]=(st,i);dc+=1;}st=i+1;}}
    let mut current_len=pn;
    for index in (0..core::cmp::min(total,dc)).rev(){
        let (ts,te)=offsets[index];if ts==te{continue;}
        let mut tok=[0u8;32];let tn=token_for(index,radix,&mut tok)?;
        let current=unsafe{&*core::ptr::addr_of!(PACK_A)};
        let next=unsafe{&mut *core::ptr::addr_of_mut!(PACK_B)};
        let mut i=0;let mut o=0;
        while i<current_len{
            let matches=current.get(i..i+tn)==Some(&tok[..tn]);
            let left=i==0||!word_byte(current[i-1]);
            let right=i+tn>=current_len||!word_byte(current[i+tn]);
            if matches&&left&&right{
                let rep=&dict_raw[ts..te];if o+rep.len()>next.len(){return Err(());}
                next[o..o+rep.len()].copy_from_slice(rep);o+=rep.len();i+=tn;
            }else{if o>=next.len(){return Err(());}next[o]=current[i];o+=1;i+=1;}
        }
        let a=unsafe{&mut *core::ptr::addr_of_mut!(PACK_A)};
        a[..o].copy_from_slice(&next[..o]);current_len=o;
    }
    let unpacked=unsafe{&*core::ptr::addr_of!(PACK_A)};
    let marker=b"var __c0rst96=\"";let cp=find_subslice(&unpacked[..current_len],marker).ok_or(())?+marker.len();
    let clen=unpacked[cp..current_len].iter().position(|b|*b==b'"').ok_or(())?;
    if clen>out.len(){return Err(());}out[..clen].copy_from_slice(&unpacked[cp..cp+clen]);Ok(clen)
}
fn base64_decode(input:&[u8],out:&mut[u8])->Result<usize,()>{
    fn v(b:u8)->Option<u8>{match b{b'A'..=b'Z'=>Some(b-b'A'),b'a'..=b'z'=>Some(b-b'a'+26),b'0'..=b'9'=>Some(b-b'0'+52),b'+'=>Some(62),b'/'=>Some(63),_=>None}}
    let mut acc=0u32;let mut bits=0;let mut n=0;
    for &b in input{if b==b'='{break;}let x=v(b).ok_or(())? as u32;acc=(acc<<6)|x;bits+=6;if bits>=8{bits-=8;if n>=out.len(){return Err(());}out[n]=(acc>>bits)as u8;n+=1;}}
    Ok(n)
}


fn run_get_pages(req: &[u8]) -> u32 {
    select_site(req);
    let raw=match extract_json_string(req,b"chapterId"){Some(v)=>v,None=>return write_error("get_pages","invalid_request","missing chapterId")};
    let mut id=[0u8;512];let idn=match json_decode(raw,&mut id){Ok(n)=>n,Err(_)=>return write_error("get_pages","invalid_request","invalid chapterId")};
    let full=&id[..idn];let tail=match full.strip_prefix(b"chapter:"){Some(v)=>v,None=>return write_error("get_pages","invalid_request","invalid chapter ID format")};
    let sep=match find_subslice(tail,b":"){Some(v)=>v,None=>return write_error("get_pages","invalid_request","invalid chapter ID format")};
    let manga=&tail[..sep];let chapter=&tail[sep+1..];
    let mut url=[0u8;1024];let mut un=0;
    if !write_bytes(&mut url,&mut un,site())||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,manga)||!write_bytes(&mut url,&mut un,b"/")||!write_bytes(&mut url,&mut un,chapter)||!write_bytes(&mut url,&mut un,b".html"){return write_error("get_pages","internal_error","URL overflow");}
    let hn=match fetch_get(&url[..un],Some(site())){Ok(n)=>n,Err(_)=>return write_error("get_pages","network_error","reader page request failed")};
    let html=&body_buf()[..hn];let mut reader=0usize;let mut cipher=[0u8;131072];
    let cn=match extract_cipher(html,&mut reader,&mut cipher){Ok(n)=>n,Err(_)=>return write_error("get_pages","parse_error","could not decode packed reader script")};
    let key=match KEYS.get(reader){Some(v)=>*v,None=>return write_error("get_pages","parse_error","unknown reader key")};
    let mut encrypted=[0u8;131072];let en=match base64_decode(&cipher[..cn],&mut encrypted){Ok(n)=>n,Err(_)=>return write_error("get_pages","parse_error","invalid cipher base64")};
    for i in 0..en{encrypted[i]^=key[i%key.len()];}
    let mut json=[0u8;131072];let jn=match base64_decode(&encrypted[..en],&mut json){Ok(n)=>n,Err(_)=>return write_error("get_pages","parse_error","invalid decoded image JSON")};
    let p=payload_buf();let mut c=0;
    if !write_bytes(p,&mut c,br#"{"chapterId":""#)||!append_json_escaped(p,&mut c,full)||!write_bytes(p,&mut c,br#"","pages":["#){return write_error("get_pages","internal_error","overflow");}
    let mut i=0usize;let mut count=0;
    while i<jn{
        while i<jn&&matches!(json[i],b' '|b'\n'|b'\r'|b'\t'|b','|b'['|b']'){i+=1;}
        if i>=jn{break;}if json[i]!=b'"'{return write_error("get_pages","parse_error","invalid image array");}
        i+=1;let start=i;let mut esc=false;
        while i<jn{if esc{esc=false;}else if json[i]==b'\\'{esc=true;}else if json[i]==b'"'{break;}i+=1;}
        if i>=jn{return write_error("get_pages","parse_error","unterminated image URL");}
        let mut decoded=[0u8;4096];let ul=match koma_source_sdk::plain_text::decode_json_content(&json[start..i],&mut decoded){Ok(v)=>v.len(),Err(_)=>return write_error("get_pages","parse_error","invalid image URL")};
        let mut absolute=[0u8;4096];let image=absolute_url(&decoded[..ul],&mut absolute);
        if count>0&&!write_bytes(p,&mut c,b","){return write_error("get_pages","internal_error","overflow");}
        let ok=write_bytes(p,&mut c,br#"{"id":"page:"#)&&write_usize(p,&mut c,count)
          &&write_bytes(p,&mut c,br#"","index":"#)&&write_usize(p,&mut c,count)
          &&write_bytes(p,&mut c,br#","image":{"kind":"url","url":""#)&&append_json_escaped(p,&mut c,image)
          &&write_bytes(p,&mut c,br#""}}"#);
        if !ok{return write_error("get_pages","internal_error","overflow");}
        count+=1;i+=1;
    }
    if count==0{return write_error("get_pages","parse_error","no images in decoded chapter");}
    if !write_bytes(p,&mut c,b"]}"){return write_error("get_pages","internal_error","overflow");}
    write_success_payload("get_pages",c)
}

// Optional operations — return not_implemented

fn run_get_listings(_req: &[u8]) -> u32 {
    let data=r#"{"listings":[{"id":"1","name":"精品榜"},{"id":"2","name":"人气榜"},{"id":"3","name":"推荐榜"},{"id":"4","name":"黑马榜"},{"id":"5","name":"最近更新"},{"id":"6","name":"新漫画"}]}"#.as_bytes();
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_listings",data.len())
}


fn run_get_manga_list(req: &[u8]) -> u32 {
    select_site(req);
    let listing=extract_json_string(req,b"listingId").unwrap_or(b"5");
    let mut url=[0u8;512];let mut un=0;
    if !write_bytes(&mut url,&mut un,site())||!write_bytes(&mut url,&mut un,b"/rank/")||!write_bytes(&mut url,&mut un,listing) {
        return write_error("get_manga_list","internal_error","URL overflow");
    }
    let hn=match fetch_get(&url[..un],Some(site())){Ok(n)=>n,Err(_)=>return write_error("get_manga_list","network_error","ranking request failed")};
    let doc=match HtmlNode::parse(&body_buf()[..hn]){Ok(v)=>v,Err(_)=>return write_error("get_manga_list","parse_error","invalid ranking HTML")};
    let p=payload_buf();let mut c=0;
    if !write_bytes(p,&mut c,br#"{"items":["#){return write_error("get_manga_list","internal_error","overflow");}
    let mut count=0;
    for i in 1..=80 {
        let mut selector=[0u8;128];let sel=match nth(b".likedata",i,&mut selector){Ok(v)=>v,Err(_)=>break};
        let card=match doc.select(sel){Ok(v)=>v,Err(_)=>continue};
        let link=match card.select(b".likeimg a"){Ok(v)=>v,Err(_)=>continue};
        let mut href=[0u8;512];let h=link.attr(b"href",&mut href).unwrap_or(b"");
        let slug=h.strip_prefix(b"/").unwrap_or(h).split(|b|*b==b'/').next().unwrap_or(b"");
        if slug.is_empty(){continue;}
        let mut title=[0u8;1024];let mut img=[0u8;2048];
        let t=card.select(b".le-t").ok().and_then(|v|v.text(&mut title).ok()).unwrap_or(b"");
        let raw_img=card.select(b".likeimg img").ok().and_then(|v|v.attr(b"data-src",&mut img).ok()).unwrap_or(b"");
        let mut cover=[0u8;4096];let cv=absolute_url(raw_img,&mut cover);
        if t.is_empty(){continue;}
        if count>0&&!write_bytes(p,&mut c,b","){return write_error("get_manga_list","internal_error","overflow");}
        let ok=write_bytes(p,&mut c,br#"{"id":"manga:"#)&&append_json_escaped(p,&mut c,slug)
          &&write_bytes(p,&mut c,br#"","title":""#)&&append_json_escaped(p,&mut c,t)
          &&write_bytes(p,&mut c,br#"","cover":{"kind":"url","url":""#)&&append_json_escaped(p,&mut c,cv)
          &&write_bytes(p,&mut c,br#""},"authors":[],"status":"unknown","contentRating":"unknown"}"#);
        if !ok{return write_error("get_manga_list","internal_error","overflow");}count+=1;
    }
    if !write_bytes(p,&mut c,br#"],"page":{"nextCursor":null,"hasMore":false}}"#){return write_error("get_manga_list","internal_error","overflow");}
    write_success_payload("get_manga_list",c)
}


fn run_get_home(_req: &[u8]) -> u32 {
    let data=br#"{"sections":[]}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_home",data.len())
}

fn run_get_filters(_req: &[u8]) -> u32 {
    let data=br#"{"filters":[]}"#;
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_filters",data.len())
}

fn run_get_settings(req: &[u8]) -> u32 {
    let data: &[u8] = if homepage_choices_supported(req) {
        r#"{"settings":[{"id":"preferredMirror","name":"\u8bbf\u95ee\u57df\u540d","kind":"select","default":"http://www.rumanhua2.com","description":"\u9009\u62e9\u641c\u7d22\u3001\u8be6\u60c5\u3001\u7ae0\u8282\u548c\u9605\u8bfb\u9875\u9762\u4f7f\u7528\u7684\u57df\u540d\u3002\u5207\u6362\u540e\u65b0\u8bf7\u6c42\u4f7f\u7528\u6240\u9009\u57df\u540d\u3002","options":[{"value":"http://www.rumanhua2.com","label":"rumanhua2.com（电脑站）"}]}]}"#.as_bytes()
    } else {
        br#"{"settings":[]}"#
    };
    if data.len() > payload_buf().len() {
        return write_error("get_settings", "internal_error", "settings overflow");
    }
    payload_buf()[..data.len()].copy_from_slice(data);
    write_success_payload("get_settings", data.len())
}

fn run_get_image_request(req: &[u8]) -> u32 {
    select_site(req);
    let raw=match extract_json_string(req,b"url"){Some(v)=>v,None=>return write_error("get_image_request","invalid_request","missing image URL")};
    let p=payload_buf();let mut c=0;
    let ok=write_bytes(p,&mut c,br#"{"url":""#)&&append_json_escaped(p,&mut c,raw)
      &&write_bytes(p,&mut c,br#"","headers":{"Referer":""#)&&append_json_escaped(p,&mut c,site())
      &&write_bytes(p,&mut c,br#"/","User-Agent":"Mozilla/5.0"}}"#);
    if !ok{return write_error("get_image_request","internal_error","overflow");}
    write_success_payload("get_image_request",c)
}


// ── Panic handler (required for no_std) ──────────────────────────
// ── WASM exports ─────────────────────────────────────────────────
// Generates: koma_source_info, koma_source_init, koma_source_search,
// koma_source_get_manga, koma_source_get_chapters, koma_source_get_pages,
// koma_source_get_listings, koma_source_get_manga_list, koma_source_get_home,
// koma_source_get_filters, koma_source_get_settings, koma_source_get_image_request,
// koma_source_free

koma_source_sdk::koma_source_exports!("rumanhua");

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
