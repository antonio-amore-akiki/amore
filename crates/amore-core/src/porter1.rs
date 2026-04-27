// porter1.rs — Porter (1980) stemmer as a Tantivy TokenFilter.
//
// Implements the classic Porter1 algorithm, which is the stemmer SQLite bundles
// for `tokenize='porter unicode61'` in FTS5.  Tantivy's built-in `en_stem` uses
// Snowball English (Porter2), which diverges for words like "deployment":
//   Porter1: m("deploy")=1 → step-4 "ment" guard fails → stem = "deployment"
//   Porter2: R2 check → strip "ment" → stem = "deploy"
// Using Porter1 is required for rank-order parity with the FTS5 baseline fixture.
//
// Reference: M. F. Porter, "An algorithm for suffix stripping",
// Program, 14(3):130-137, 1980.  ASCII-only; non-ASCII tokens pass through.

use tantivy::tokenizer::{Token, TokenFilter, TokenStream, Tokenizer};

/// Porter1 (1980) stemmer TokenFilter for Tantivy.
#[derive(Clone)]
pub(crate) struct Porter1Filter;

impl TokenFilter for Porter1Filter {
    type Tokenizer<T: Tokenizer> = Porter1Tokenizer<T>;

    fn transform<T: Tokenizer>(self, inner: T) -> Porter1Tokenizer<T> {
        Porter1Tokenizer { inner }
    }
}

#[derive(Clone)]
pub(crate) struct Porter1Tokenizer<T> {
    inner: T,
}

impl<T: Tokenizer> Tokenizer for Porter1Tokenizer<T> {
    type TokenStream<'a> = Porter1Stream<T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        Porter1Stream {
            tail: self.inner.token_stream(text),
            buf: String::new(),
        }
    }
}

pub(crate) struct Porter1Stream<T> {
    tail: T,
    buf: String,
}

impl<T: TokenStream> TokenStream for Porter1Stream<T> {
    fn advance(&mut self) -> bool {
        if !self.tail.advance() {
            return false;
        }
        let tok: &mut Token = self.tail.token_mut();
        if tok.text.chars().all(|c| c.is_ascii_alphabetic()) {
            self.buf.clear();
            self.buf.push_str(&tok.text);
            stem(&mut self.buf);
            tok.text = self.buf.clone();
        }
        true
    }

    fn token(&self) -> &Token {
        self.tail.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.tail.token_mut()
    }
}

// ---------------------------------------------------------------------------
// Porter1 algorithm
// ---------------------------------------------------------------------------

fn is_c(w: &[u8], i: usize) -> bool {
    match w[i] {
        b'a' | b'e' | b'i' | b'o' | b'u' => false,
        b'y' => i == 0 || !is_c(w, i - 1),
        _ => true,
    }
}

fn measure(w: &[u8], end: usize) -> usize {
    let mut i = 0;
    while i < end && is_c(w, i) { i += 1; }
    let mut count = 0;
    loop {
        while i < end && !is_c(w, i) { i += 1; }
        if i >= end { break; }
        count += 1;
        while i < end && is_c(w, i) { i += 1; }
    }
    count
}

fn vowel_in(w: &[u8], end: usize) -> bool {
    (0..end).any(|i| !is_c(w, i))
}

fn double_c(w: &[u8], end: usize) -> bool {
    end >= 2 && w[end - 1] == w[end - 2] && is_c(w, end - 1)
}

fn cvc(w: &[u8], end: usize) -> bool {
    if end < 3 { return false; }
    let c = w[end - 1];
    c != b'w' && c != b'x' && c != b'y'
        && is_c(w, end - 1) && !is_c(w, end - 2) && is_c(w, end - 3)
}

/// Apply Porter1 stemming in-place to a lowercase ASCII-alphabetic string.
pub(crate) fn stem(s: &mut String) {
    if s.len() <= 2 { return; }
    // SAFETY: all writes store valid lowercase ASCII bytes into a valid UTF-8 slice.
    let w = unsafe { s.as_bytes_mut() };
    let n = w.len();
    let n = s1a(w, n);
    let n = s1b(w, n);
    let n = s1c(w, n);
    let n = s2(w, n);
    let n = s3(w, n);
    let n = s4(w, n);
    let n = s5a(w, n);
    let n = s5b(w, n);
    s.truncate(n);
}

fn s1a(w: &mut [u8], end: usize) -> usize {
    if w[..end].ends_with(b"sses") { end - 2 }
    else if w[..end].ends_with(b"ies") { w[end - 3] = b'i'; end - 2 }
    else if w[..end].ends_with(b"ss") { end }
    else if w[..end].ends_with(b"s") { end - 1 }
    else { end }
}

fn s1b(w: &mut [u8], end: usize) -> usize {
    if w[..end].ends_with(b"eed") {
        let b = end - 3;
        return if measure(w, b) > 0 { end - 1 } else { end };
    }
    if w[..end].ends_with(b"ed") {
        let b = end - 2;
        if vowel_in(w, b) { return s1b2(w, b); }
        return end;
    }
    if w[..end].ends_with(b"ing") {
        let b = end - 3;
        if vowel_in(w, b) { return s1b2(w, b); }
    }
    end
}

fn s1b2(w: &mut [u8], end: usize) -> usize {
    if w[..end].ends_with(b"at") || w[..end].ends_with(b"bl") || w[..end].ends_with(b"iz") {
        w[end] = b'e'; return end + 1;
    }
    if double_c(w, end) && !matches!(w[end - 1], b'l' | b's' | b'z') { return end - 1; }
    if measure(w, end) == 1 && cvc(w, end) { w[end] = b'e'; return end + 1; }
    end
}

fn s1c(w: &mut [u8], end: usize) -> usize {
    if w[..end].ends_with(b"y") && vowel_in(w, end - 1) { w[end - 1] = b'i'; }
    end
}

macro_rules! rep {
    ($fn:ident, $w:ident, $end:ident, $($suf:literal => $rep:literal),+ $(,)?) => {
        fn $fn(w: &mut [u8], end: usize) -> usize {
            $(
                if w[..end].ends_with($suf) {
                    let b = end - $suf.len();
                    if measure(w, b) > 0 {
                        w[b..b + $rep.len()].copy_from_slice($rep);
                        return b + $rep.len();
                    }
                    return end;
                }
            )+
            end
        }
    };
}

rep!(s2, _w, _end,
    b"ational" => b"ate",
    b"tional"  => b"tion",
    b"enci"    => b"ence",
    b"anci"    => b"ance",
    b"izer"    => b"ize",
    b"abli"    => b"able",
    b"alli"    => b"al",
    b"entli"   => b"ent",
    b"eli"     => b"e",
    b"ousli"   => b"ous",
    b"ization" => b"ize",
    b"ation"   => b"ate",
    b"ator"    => b"ate",
    b"alism"   => b"al",
    b"iveness" => b"ive",
    b"fulness" => b"ful",
    b"ousness" => b"ous",
    b"aliti"   => b"al",
    b"iviti"   => b"ive",
    b"biliti"  => b"ble",
);

fn s3(w: &mut [u8], end: usize) -> usize {
    macro_rules! m3rep {
        ($suf:literal, $rep:literal) => {
            if w[..end].ends_with($suf) {
                let b = end - $suf.len();
                if measure(w, b) > 0 {
                    w[b..b + $rep.len()].copy_from_slice($rep);
                    return b + $rep.len();
                }
                return end;
            }
        };
        ($suf:literal) => {
            if w[..end].ends_with($suf) {
                let b = end - $suf.len();
                if measure(w, b) > 0 { return b; }
                return end;
            }
        };
    }
    m3rep!(b"icate", b"ic"); m3rep!(b"ative"); m3rep!(b"alize", b"al");
    m3rep!(b"iciti", b"ic"); m3rep!(b"ical",  b"ic"); m3rep!(b"ful"); m3rep!(b"ness");
    end
}

fn s4(w: &mut [u8], end: usize) -> usize {
    macro_rules! strip {
        ($suf:literal) => {
            if w[..end].ends_with($suf) {
                let b = end - $suf.len();
                if measure(w, b) > 1 { return b; }
                return end;
            }
        };
    }
    strip!(b"al"); strip!(b"ance"); strip!(b"ence"); strip!(b"er"); strip!(b"ic");
    strip!(b"able"); strip!(b"ible"); strip!(b"ant"); strip!(b"ement"); strip!(b"ment");
    strip!(b"ent");
    if w[..end].ends_with(b"ion") {
        let b = end - 3;
        if b > 0 && measure(w, b) > 1 && (w[b - 1] == b's' || w[b - 1] == b't') { return b; }
        return end;
    }
    strip!(b"ou"); strip!(b"ism"); strip!(b"ate"); strip!(b"iti");
    strip!(b"ous"); strip!(b"ive"); strip!(b"ize");
    end
}

fn s5a(w: &mut [u8], end: usize) -> usize {
    if w[..end].ends_with(b"e") {
        let b = end - 1;
        if measure(w, b) > 1 { return b; }
        if measure(w, b) == 1 && !cvc(w, b) { return b; }
    }
    end
}

fn s5b(w: &[u8], end: usize) -> usize {
    if w[..end].ends_with(b"ll") && measure(w, end) > 1 { end - 1 } else { end }
}
