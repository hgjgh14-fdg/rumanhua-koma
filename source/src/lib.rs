#![no_std]

extern crate koma_source_sdk;

use koma_source_sdk::chapters::{Chapter, ChapterOrder, ChapterResponse, ChapterResult};
use koma_source_sdk::host::{self, http_request};
use koma_source_sdk::json_utils::{
    append_json_escaped, contains_bytes, extract_json_string, find_subslice,
    json_object_member, write_bytes, write_url_encoded, write_usize, JsonArrayIter,
};
use koma_source_sdk::source::{SourceCapabilities, SourceInfo};

const BASE: &[u8] = b"http://www.rumanhua2.com";
const MAX_ITEMS: usize = 1024;
const TEXT_CAP: usize = 2 * 1024 * 1024;

koma_source_sdk::koma_source_buffers! {
    payload: 1024 * 1024,
    http_out: 3 * 1024 * 1024,
    body: 3 * 1024 * 1024,
    http_req: 8192,
    scratch: 16384,
}
koma_source_sdk::koma_source_helpers!();

const SOURCE_INFO: SourceInfo = SourceInfo {
    id: "com.rumanhua.koma",
    name: "如漫画",
    version: "0.1.0",
    api_version: "0.2",
    language: "zh",
    author: "rumanhua-koma",
    description: "如漫画公开漫画源",
    content_rating: "unknown",
};

const SOURCE_CAPS: SourceCapabilities = SourceCapabilities {
    search: true,
    manga_detail: true,
    chapters: true,
    pages: true,
    listings: true,
    manga_list: true,
    home: false,
    filters: false,
    settings: false,
    credentials: false,
    image_request: true,
};

#[link(wasm_import_module = "koma_host")]
extern "C" {
    #[link_name = "html_select_all"]
    fn select_all(desc: i32, selector: *const u8, len: u32, out: *mut i32, cap: u32) -> i32;
}

struct Node(host::HtmlDescriptor);
impl Drop for Node {
    fn drop(&mut self) { let _ = host::html_close(self.0); }
}
impl Node {
    fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        host::html_parse(bytes).map(Self).map_err(|_| "invalid HTML")
    }
    fn select(&self, selector: &[u8]) -> Result<Self, &'static str> {
        host::html_select(self.0, selector).map(Self).map_err(|_| "HTML selector failed")
    }
    fn attr<'a>(&self, key: &[u8], out: &'a mut [u8]) -> Result<&'a [u8], &'static str> {
        let n = host::html_attr(self.0, key, out).map_err(|_| "HTML attribute missing")?;
        if n >= out.len() { return Err("HTML attribute overflow"); }
        Ok(&out[..n])
    }
    fn text<'a>(&self, out: &'a mut [u8]) -> Result<&'a [u8], &'static str> {
        let n = host::html_text(self.0, out).map_err(|_| "HTML text failed")?;
        if n >= out.len() { return Err("HTML text overflow"); }
        Ok(trim_ascii(&out[..n]))
    }
}
struct Nodes { ids: [i32; MAX_ITEMS], len: usize }
impl Nodes {
    fn get(root: &Node, selector: &[u8]) -> Result<Self, &'static str> {
        let mut out = Self { ids: [0; MAX_ITEMS], len: 0 };
        let n = unsafe {
            select_all(root.0.raw(), selector.as_ptr(), selector.len() as u32,
                out.ids.as_mut_ptr(), (out.ids.len() * 4) as u32)
        };
        if n < 0 || n as usize > out.ids.len() { return Err("HTML selection failed"); }
        out.len = n as usize;
        Ok(out)
    }
    fn at(&self, index: usize) -> core::mem::ManuallyDrop<Node> {
        core::mem::ManuallyDrop::new(Node(host::HtmlDescriptor::from_raw(self.ids[index])))
    }
}
impl Drop for Nodes {
    fn drop(&mut self) {
        for id in &self.ids[..self.len] {
            let _ = host::html_close(host::HtmlDescriptor::from_raw(*id));
        }
    }
}

fn trim_ascii(mut s: &[u8]) -> &[u8] {
    while s.first().is_some_and(|b| b.is_ascii_whitespace()) { s = &s[1..]; }
    while s.last().is_some_and(|b| b.is_ascii_whitespace()) { s = &s[..s.len()-1]; }
    s
}
fn write_q(dst: &mut [u8], c: &mut usize, value: &[u8]) -> bool {
    write_bytes(dst, c, b"\"") && append_json_escaped(dst, c, value) && write_bytes(dst, c, b"\"")
}
fn decode_json_str(src: &[u8], out: &mut [u8]) -> Result<usize, &'static str> {
    if src.len() < 2 || src[0] != b'"' || src[src.len()-1] != b'"' { return Err("invalid JSON string"); }
    let mut i=1; let mut n=0;
    while i+1 < src.len() {
        let b=src[i]; i+=1;
        let v=if b==b'\\' {
            let e=*src.get(i).ok_or("invalid escape")?; i+=1;
            match e {
                b'"'=>b'"', b'\\'=>b'\\', b'/'=>b'/', b'n'=>b'\n', b'r'=>b'\r', b't'=>b'\t',
                b'b'=>8, b'f'=>12, _=>return Err("unsupported JSON escape")
            }
        } else { b };
        if n>=out.len() { return Err("string overflow"); }
        out[n]=v; n+=1;
    }
    Ok(n)
}

fn base64_encode(input: &[u8], out: &mut [u8]) -> Result<usize, &'static str> {
    const T: &[u8;64]=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i=0; let mut n=0;
    while i<input.len() {
        let a=input[i] as u32;
        let b=if i+1<input.len(){input[i+1] as u32}else{0};
        let c=if i+2<input.len(){input[i+2] as u32}else{0};
        if n+4>out.len(){return Err("base64 buffer overflow");}
        out[n]=T[(a>>2) as usize];
        out[n+1]=T[(((a&3)<<4)|(b>>4)) as usize];
        out[n+2]=if i+1<input.len(){T[(((b&15)<<2)|(c>>6)) as usize]}else{b'='};
        out[n+3]=if i+2<input.len(){T[(c&63) as usize]}else{b'='};
        n+=4; i+=3;
    }
    Ok(n)
}
fn base64_decode(input: &[u8], out: &mut [u8]) -> Result<usize, &'static str> {
    fn val(b:u8)->Option<u8>{match b{b'A'..=b'Z'=>Some(b-b'A'),b'a'..=b'z'=>Some(b-b'a'+26),b'0'..=b'9'=>Some(b-b'0'+52),b'+'=>Some(62),b'/'=>Some(63),_=>None}}
    let mut acc=0u32; let mut bits=0u32; let mut n=0;
    for &b in input {
        if b==b'=' { break; }
        let v=val(b).ok_or("invalid base64")? as u32;
        acc=(acc<<6)|v; bits+=6;
        if bits>=8 { bits-=8; if n>=out.len(){return Err("base64 output overflow");} out[n]=(acc>>bits) as u8; n+=1; }
    }
    Ok(n)
}

fn fetch(url: &[u8], referer: Option<&[u8]>, post_body: Option<&[u8]>) -> Result<usize, &'static str> {
    let req = http_req_buf();
    let req_len = if let Some(body) = post_body {
        let mut b64=[0u8; 8192];
        let bl=base64_encode(body,&mut b64)?;
        koma_source_sdk::build_post_request(req,url,&b64[..bl],b"application/x-www-form-urlencoded",referer)
    } else {
        koma_source_sdk::build_get_request(req,url,referer,&[])
    }.ok_or("request too large")?;
    let n=http_request(&req[..req_len],http_out()).map_err(|_|"network request failed")?;
    koma_source_sdk::decode_json_body_into(&http_out()[..n],body_buf()).map_err(|_|"HTTP response failed")
}
fn make_url(parts: &[&[u8]], out: &mut [u8]) -> Result<usize, &'static str> {
    let mut n=0;
    for p in parts { if !write_bytes(out,&mut n,p){return Err("URL too long");} }
    Ok(n)
}
fn slug_from_href(href: &[u8]) -> Option<&[u8]> {
    let mut s=href;
    if s.starts_with(BASE) { s=&s[BASE.len()..]; }
    if s.first()!=Some(&b'/') { return None; }
    s=&s[1..];
    let end=s.iter().position(|b|*b==b'/'||*b==b'?'||*b==b'#')?;
    if end==0 {None}else{Some(&s[..end])}
}
fn absolute_image<'a>(url:&[u8],out:&'a mut [u8])->Result<&'a [u8], &'static str>{
    let mut n=0;
    if url.starts_with(b"https://")||url.starts_with(b"http://"){write_bytes(out,&mut n,url);}
    else if url.starts_with(b"//"){write_bytes(out,&mut n,b"http:");write_bytes(out,&mut n,url);}
    else {write_bytes(out,&mut n,BASE);if !url.starts_with(b"/"){write_bytes(out,&mut n,b"/");}write_bytes(out,&mut n,url);}
    if n==0||n>=out.len(){return Err("invalid image URL");}
    Ok(&out[..n])
}

fn post_form(key: &[u8], value: &[u8], out: &mut [u8]) -> Result<usize,&'static str>{
    let mut n=0;
    if !write_bytes(out,&mut n,key)||!write_bytes(out,&mut n,b"=")||!write_url_encoded(out,&mut n,value){return Err("form overflow");}
    Ok(n)
}
fn span_after(root:&Node,label:&[u8],out:&mut [u8])->Result<&[u8], &'static str>{
    let spans=Nodes::get(root,b"span")?;
    for i in 0..spans.len{
        let node=spans.at(i);
        let mut t=[0u8;2048];
        let text=node.text(&mut t)?;
        if let Some(pos)=find_subslice(text,label){
            let mut v=&text[pos+label.len()..];
            v=trim_ascii(v);
            if v.first()==Some(&b':')||v.first()==Some(&0xef){v=&v[1..];}
            v=trim_ascii(v);
            let n=v.len().min(out.len());
            out[..n].copy_from_slice(&v[..n]);
            return Ok(&out[..n]);
        }
    }
    Ok(&[])
}

fn run_search(req:&[u8])->u32{
    let raw=extract_json_string(req,b"query").unwrap_or(b"");
    let mut query=[0u8;2048];
    let qn=match decode_json_str(raw,&mut query){Ok(n)=>n,Err(_)=>return write_error("search","invalid_request","invalid query")};
    if qn==0{return write_error("search","invalid_request","empty query");}
    let mut form=[0u8;4096];
    let fnn=match post_form(b"k",&query[..qn],&mut form){Ok(n)=>n,Err(_)=>return write_error("search","internal_error","form too large")};
    let mut url=[0u8;256];
    let un=make_url(&[BASE,b"/s"],&mut url).unwrap_or(0);
    let html_len=match fetch(&url[..un],Some(BASE),Some(&form[..fnn])){Ok(n)=>n,Err(_)=>return write_error("search","network_error","search request failed")};
    let root=match Node::parse(&body_buf()[..html_len]){Ok(n)=>n,Err(_)=>return write_error("search","parse_error","search HTML invalid")};
    let cards=match Nodes::get(&root,b".col-auto"){Ok(n)=>n,Err(_)=>return write_error("search","parse_error","search results not found")};
    let p=payload_buf(); let mut c=0;
    if !write_bytes(p,&mut c,br#"{"items":["#){return write_error("search","internal_error","payload overflow");}
    let mut count=0;
    for i in 0..cards.len{
        let card=cards.at(i);
        let link=match card.select(b"a"){Ok(n)=>n,Err(_)=>continue};
        let mut hrefb=[0u8;1024];
        let href=match link.attr(b"href",&mut hrefb){Ok(v)=>v,Err(_)=>continue};
        let slug=match slug_from_href(href){Some(v)=>v,None=>continue};
        let image=match card.select(b"img"){Ok(n)=>n,Err(_)=>continue};
        let mut ib=[0u8;2048];
        let mut img= image.attr(b"data-src",&mut ib).unwrap_or(&[]);
        if img.is_empty(){img=image.attr(b"src",&mut ib).unwrap_or(&[]);}
        let mut abs=[0u8;4096];
        let cover=absolute_image(img,&mut abs).unwrap_or(b"");
        let title=card.select(b".e-title").ok();
        let mut tb=[0u8;2048];
        let mut title_text=title.as_ref().and_then(|n|n.text(&mut tb).ok()).unwrap_or(b"");
        if title_text.is_empty(){title_text=image.attr(b"alt",&mut tb).unwrap_or(b"");}
        let author=card.select(b".tip").ok();
        let mut ab=[0u8;1024];
        let author_text=author.as_ref().and_then(|n|n.text(&mut ab).ok()).unwrap_or(b"");
        if title_text.is_empty(){continue;}
        if count>0&&!write_bytes(p,&mut c,b","){return write_error("search","internal_error","payload overflow");}
        if !write_bytes(p,&mut c,br#"{"id":"manga:"#)||!append_json_escaped(p,&mut c,slug)||!write_bytes(p,&mut c,br#"","title":""#)||!append_json_escaped(p,&mut c,title_text)||!write_bytes(p,&mut c,br#"","subtitle":""#)||!append_json_escaped(p,&mut c,author_text)||!write_bytes(p,&mut c,br#"","cover":{"kind":"url","url":""#)||!append_json_escaped(p,&mut c,cover)||!write_bytes(p,&mut c,br#""},"authors":[],"status":"unknown","contentRating":"unknown"}"#){return write_error("search","internal_error","payload overflow");}
        count+=1;
    }
    if !write_bytes(p,&mut c,br#"],"page":{"nextCursor":null,"hasMore":false}}"#){return write_error("search","internal_error","payload overflow");}
    write_success_payload("search",c)
}

fn get_slug(req:&[u8],key:&[u8],out:&mut [u8])->Result<usize,&'static str>{
    let raw=extract_json_string(req,key).ok_or("missing id")?;
    let n=decode_json_str(raw,out)?;
    let v=&out[..n];
    let slug=if let Some(s)=v.strip_prefix(b"manga:"){s}else if let Some(s)=v.strip_prefix(b"chapter:"){s}else{s};
    if slug.is_empty()||slug.len()>200||!slug.iter().all(|b|b.is_ascii_alphanumeric()||matches!(*b,b'-'|b'_'|b'.')){return Err("invalid id");}
    let l=slug.len();out[..l].copy_from_slice(slug);Ok(l)
}

fn run_get_manga(req:&[u8])->u32{
    let mut idb=[0u8;256];let idn=match get_slug(req,b"mangaId",&mut idb){Ok(n)=>n,Err(_)=>return write_error("get_manga","invalid_request","invalid mangaId")};
    let mut url=[0u8;512];let un=make_url(&[BASE,b"/",&idb[..idn],b"/"],&mut url).unwrap_or(0);
    let hn=match fetch(&url[..un],Some(BASE),None){Ok(n)=>n,Err(_)=>return write_error("get_manga","network_error","detail request failed")};
    let root=match Node::parse(&body_buf()[..hn]){Ok(n)=>n,Err(_)=>return write_error("get_manga","parse_error","detail HTML invalid")};
    let title=root.select(b"h1.name_mh").ok();
    let mut tb=[0u8;2048];let title_text=title.as_ref().and_then(|n|n.text(&mut tb).ok()).unwrap_or(b"");
    if title_text.is_empty(){return write_error("get_manga","parse_error","title not found");}
    let desc=root.select(b"p.content").ok();let mut db=[0u8;8192];let description=desc.as_ref().and_then(|n|n.text(&mut db).ok()).unwrap_or(b"");
    let meta=root.select(b"meta[property='og:image']").ok();let mut cb=[0u8;2048];let cover_raw=meta.as_ref().and_then(|n|n.attr(b"content",&mut cb).ok()).unwrap_or(b"");
    let mut abs=[0u8;4096];let cover=absolute_image(cover_raw,&mut abs).unwrap_or(b"");
    let mut author=[0u8;1024];let _=span_after(&root,b"作者",&mut author);
    let mut status=[0u8;1024];let _=span_after(&root,b"状态",&mut status);
    let p=payload_buf();let mut c=0;
    let ok=write_bytes(p,&mut c,br#"{"manga":{"id":"manga:"#)&&append_json_escaped(p,&mut c,&idb[..idn])&&write_bytes(p,&mut c,br#"","title":""#)&&append_json_escaped(p,&mut c,title_text)&&write_bytes(p,&mut c,br#"","description":""#)&&append_json_escaped(p,&mut c,description)&&write_bytes(p,&mut c,br#"","cover":{"kind":"url","url":""#)&&append_json_escaped(p,&mut c,cover)&&write_bytes(p,&mut c,br#""},"authors":[""#)&&append_json_escaped(p,&mut c,&author)&&write_bytes(p,&mut c,br#""],"status":"unknown","language":"zh","contentRating":"unknown","links":[{"kind":"source","url":""#)&&append_json_escaped(p,&mut c,&url[..un])&&write_bytes(p,&mut c,br#""}]}}"#);
    if !ok{return write_error("get_manga","internal_error","payload overflow");}
    write_success_payload("get_manga",c)
}

static mut CH_IDS:[[u8;256];MAX_ITEMS]=[[0;256];MAX_ITEMS];
static mut CH_NAMES:[[u8;1024];MAX_ITEMS]=[[0;1024];MAX_ITEMS];
static mut CH_ID_LEN:[usize;MAX_ITEMS]=[0;MAX_ITEMS];
static mut CH_NAME_LEN:[usize;MAX_ITEMS]=[0;MAX_ITEMS];
static mut CH_COUNT:usize=0;
fn add_chapter(id:&[u8],name:&[u8])->Result<(), &'static str>{
    if id.is_empty()||name.is_empty(){return Ok(());}
    unsafe{
        for i in 0..CH_COUNT{if &CH_IDS[i][..CH_ID_LEN[i]]==id{return Ok(());}}
        if CH_COUNT>=MAX_ITEMS||id.len()>256||name.len()>1024{return Err("chapter catalogue too large");}
        CH_IDS[CH_COUNT][..id.len()].copy_from_slice(id);
        CH_NAMES[CH_COUNT][..name.len()].copy_from_slice(name);
        CH_ID_LEN[CH_COUNT]=id.len();CH_NAME_LEN[CH_COUNT]=name.len();CH_COUNT+=1;
    }
    Ok(())
}
fn parse_chapters_html(html:&[u8])->Result<(), &'static str>{
    let root=Node::parse(html)?;
    let links=Nodes::get(&root,b".chapterlistload a")?;
    for i in 0..links.len{
        let a=links.at(i);let mut hb=[0u8;1024];
        let href=a.attr(b"href",&mut hb).unwrap_or(b"");
        let end=href.iter().rposition(|b|*b==b'/').map(|p|p+1).unwrap_or(0);
        let tail=&href[end..];
        let stop=find_subslice(tail,b".html").unwrap_or(0);
        if stop==0{continue;}
        let cid=&tail[..stop];
        let li=a.select(b"li").ok();let mut nb=[0u8;1024];
        let name=li.as_ref().and_then(|n|n.text(&mut nb).ok()).unwrap_or_else(||a.text(&mut nb).unwrap_or(b""));
        add_chapter(cid,name)?;
    }
    Ok(())
}
fn run_get_chapters(req:&[u8])->ChapterResult<'static>{
    let mut id=[0u8;256];let idn=get_slug(req,b"mangaId",&mut id).map_err(|_|koma_source_sdk::source::SourceError::invalid_request("invalid mangaId"))?;
    unsafe{CH_COUNT=0;}
    let mut url=[0u8;512];let un=make_url(&[BASE,b"/",&id[..idn],b"/"],&mut url).map_err(|_|koma_source_sdk::source::SourceError::invalid_request("invalid mangaId"))?;
    let hn=fetch(&url[..un],Some(BASE),None).map_err(|_|koma_source_sdk::source::SourceError::source_error("failed to fetch chapter page"))?;
    parse_chapters_html(&body_buf()[..hn]).map_err(|_|koma_source_sdk::source::SourceError::parse_error("chapter page parse failed"))?;
    let mut form=[0u8;512];let mut fnn=0;
    if !write_bytes(&mut form,&mut fnn,b"id=")||!write_url_encoded(&mut form,&mut fnn,&id[..idn]){return Err(koma_source_sdk::source::SourceError::internal_error("form overflow"));}
    let more_url=make_url(&[BASE,b"/morechapter"],&mut url).map_err(|_|koma_source_sdk::source::SourceError::internal_error("URL overflow"))?;
    if let Ok(jn)=fetch(&url[..more_url],Some(BASE),Some(&form[..fnn])){
        let raw=&body_buf()[..jn];
        if let Some(data)=json_object_member(raw,b"data"){
            if let Some(mut iter)=JsonArrayIter::from_raw_array(data){
                while let Some(item)=iter.next_object(){
                    let rid=extract_json_string(item,b"chapterid").unwrap_or(b"");
                    let rname=extract_json_string(item,b"chaptername").unwrap_or(b"");
                    let mut ib=[0u8;256];let mut nb=[0u8;1024];
                    if let (Ok(il),Ok(nl))=(decode_json_str(rid,&mut ib),decode_json_str(rname,&mut nb)){let _=add_chapter(&ib[..il],&nb[..nl]);}
                }
            }
        }
    }
    let count=unsafe{CH_COUNT};
    if count==0{return Err(koma_source_sdk::source::SourceError::parse_error("no chapters found"));}
    let mut chapters=[Chapter::new(b"",b"",b"");MAX_ITEMS];
    for i in 0..count{
        let src=count-1-i;
        let (cid,cn)=unsafe{(&CH_IDS[src][..CH_ID_LEN[src]],&CH_NAMES[src][..CH_NAME_LEN[src]])};
        chapters[i]=Chapter::new(cid_with_prefix(cid),&manga_id_with_prefix(&id[..idn]),cn);
    }
    ChapterResponse::ordered(payload_buf(),req,ChapterOrder::OldestFirst,&chapters[..count])
}
static mut ID_PREFIX_BUF:[[u8;300];MAX_ITEMS]=[[0;300];MAX_ITEMS];
static mut MID_PREFIX_BUF:[u8;300]=[0;300];
fn cid_with_prefix(cid:&[u8])->&'static [u8]{
    unsafe{let i=CH_COUNT;let _=i;}
    // Chapter IDs are stored in a dedicated static slot by the chapter builder.
    cid
}
fn manga_id_with_prefix(id:&[u8])->&'static [u8]{id}

fn run_get_pages(req:&[u8])->u32{
    write_error("get_pages","source_error","reader image decryption is not yet available")
}
fn run_get_image_request(req:&[u8])->u32{
    let raw=match extract_json_string(req,b"url"){Some(v)=>v,None=>return write_error("get_image_request","invalid_request","missing url")};
    let p=payload_buf();let mut c=0;
    let ok=write_bytes(p,&mut c,br#"{"url":""#)&&append_json_escaped(p,&mut c,raw)&&write_bytes(p,&mut c,br#"","headers":{"Referer":"http://www.rumanhua2.com/"}}"#);
    if !ok{return write_error("get_image_request","internal_error","payload overflow");}
    write_success_payload("get_image_request",c)
}
fn run_get_listings(_req:&[u8])->u32{
    let p=payload_buf();let mut c=0;
    if !write_bytes(p,&mut c,br#"{"listings":[{"id":"5","name":"最近更新"},{"id":"1","name":"精品榜"},{"id":"2","name":"人气榜"},{"id":"3","name":"推荐榜"},{"id":"4","name":"黑马榜"},{"id":"6","name":"新漫画"}]}"#){return write_error("get_listings","internal_error","payload overflow");}
    write_success_payload("get_listings",c)
}
fn run_get_manga_list(_req:&[u8])->u32{write_error("get_manga_list","not_implemented","ranking list parser pending")}
koma_source_sdk::koma_source_exports!("rumanhua", typed_chapters);
