//! Pure character-encoding engine (no Arrow, no RPC): detection, decoding,
//! encoding, mojibake repair, and validity checks over `&[u8]` / `&str`.
//!
//! ## Libraries
//!
//! - [`chardetng`](https://docs.rs/chardetng) — the encoding *detector* Firefox
//!   uses for legacy/unlabelled bytes (a Rust port of Mozilla's `chardet`-style
//!   heuristic). It is a guess: it returns a single most-likely [`Encoding`]
//!   from a byte sample; it has no probability output.
//! - [`encoding_rs`](https://docs.rs/encoding_rs) — the WHATWG Encoding Standard
//!   codec library Firefox uses. It maps every web-platform label to a static
//!   [`Encoding`] and does the actual decode/encode, including the lossy
//!   U+FFFD / HTML-numeric-reference handling for unmappable input.
//!
//! ## Detection order
//!
//! [`detect`] checks for a Unicode BOM first (UTF-8 / UTF-16LE / UTF-16BE) and
//! trusts it — a BOM is an explicit, unambiguous declaration. With no BOM it
//! hands the bytes to `chardetng`. Empty input has no encoding to detect and
//! returns `None`.
//!
//! ## Confidence proxy
//!
//! `chardetng` is heuristic and exposes no probability. [`confidence`] therefore
//! derives a value in `[0, 1]` from the *decode result*, which is the property
//! callers actually care about ("can these bytes be read cleanly as the detected
//! encoding?"):
//!
//! - `1.0`  — bytes decode with **no** U+FFFD replacement characters (lossless),
//!   or an explicit BOM was present.
//! - `1 - replaced/total` — otherwise, scaled down by the fraction of decoded
//!   characters that came out as U+FFFD.
//! - `0.0`  — empty input.

use encoding_rs::Encoding;

/// Detect the most likely encoding label for `bytes`.
///
/// Returns `None` for empty input. Checks for a UTF-8/UTF-16 BOM first, then
/// falls back to the `chardetng` heuristic.
pub fn detect(bytes: &[u8]) -> Option<&'static str> {
    if bytes.is_empty() {
        return None;
    }
    Some(detect_encoding(bytes).name())
}

/// Detect the most likely [`Encoding`] for `bytes` (non-empty assumed by callers
/// that care about the empty case). BOM first, then `chardetng`.
fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    if let Some((enc, _bom_len)) = Encoding::for_bom(bytes) {
        return enc;
    }
    let mut detector = chardetng::EncodingDetector::new();
    // `true` = we have fed the detector the whole document (no more bytes
    // coming), which lets it use end-of-input signals.
    detector.feed(bytes, true);
    // No top-level domain hint, and don't allow UTF-8 to win purely on the
    // absence of evidence (`allow_utf8 = true` lets valid UTF-8 be reported).
    detector.guess(None, true)
}

/// A confidence proxy in `[0, 1]` for reading `bytes` as the detected encoding.
/// See the module docs for how it is derived. `0.0` for empty input.
pub fn confidence(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    if Encoding::for_bom(bytes).is_some() {
        return 1.0;
    }
    let enc = detect_encoding(bytes);
    let (text, _, had_errors) = enc.decode(bytes);
    if !had_errors {
        return 1.0;
    }
    let total = text.chars().count();
    if total == 0 {
        return 0.0;
    }
    let replaced = text.chars().filter(|&c| c == '\u{FFFD}').count();
    1.0 - (replaced as f64 / total as f64)
}

/// Detect the encoding of `bytes` and decode to a UTF-8 [`String`]. Returns
/// `None` for empty input. Undecodable bytes become U+FFFD.
pub fn to_utf8(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let enc = detect_encoding(bytes);
    let (text, _used, _had_errors) = enc.decode(bytes);
    Some(text.into_owned())
}

/// Decode `bytes` to a UTF-8 [`String`] using an **explicit** encoding label (no
/// detection). Returns `Err` with the offending label if it is not a recognized
/// WHATWG encoding label. Undecodable bytes within a known encoding become
/// U+FFFD.
pub fn to_utf8_from(bytes: &[u8], label: &str) -> Result<String, String> {
    let enc = Encoding::for_label(label.as_bytes())
        .ok_or_else(|| format!("unknown encoding label: {label:?}"))?;
    let (text, _used, _had_errors) = enc.decode(bytes);
    Ok(text.into_owned())
}

/// Encode a UTF-8 string into the named encoding's bytes (for export). Returns
/// `Err` if the label is unknown.
///
/// Per `encoding_rs`, characters the target encoding cannot represent are
/// emitted as HTML numeric character references (e.g. `&#1234;`) for the legacy
/// single-/multi-byte encodings; the UTF-8/UTF-16 encoders are lossless. This
/// matches what a browser does when form-submitting in a legacy encoding.
pub fn transcode(text: &str, label: &str) -> Result<Vec<u8>, String> {
    let enc = Encoding::for_label(label.as_bytes())
        .ok_or_else(|| format!("unknown encoding label: {label:?}"))?;
    let (bytes, _used, _had_unmappable) = enc.encode(text);
    Ok(bytes.into_owned())
}

/// Whether `bytes` is already valid UTF-8.
pub fn is_valid_utf8(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
}

/// Repair the classic double-encoding mojibake where UTF-8 bytes were decoded as
/// Latin-1 / Windows-1252 and then re-stored as UTF-8 (so `é` shows as `Ã©`,
/// `"` as `â€œ`, etc.).
///
/// ## Heuristic
///
/// The input `text` is a Rust `&str`, i.e. already valid UTF-8. Mojibake of this
/// kind means the *intended* bytes are recoverable by taking each character's
/// Windows-1252 byte and re-decoding the resulting byte stream as UTF-8:
///
/// 1. Re-encode `text` as Windows-1252. If any character isn't representable
///    (returns an unmappable / HTML-ref escape), the text isn't pure 1252-mojibake
///    → **no-op** (return the input unchanged).
/// 2. Decode those bytes as UTF-8. If it isn't valid UTF-8, **no-op**.
/// 3. Only accept the repair if it actually *improved* things — specifically, if
///    the candidate contains fewer of the tell-tale mojibake marker characters
///    (`Ã Â â€ etc.`) than the input. This stops it from mangling text that was
///    never mojibake (e.g. a legitimately Windows-1252-looking string).
///
/// When it can't confidently improve the text, it returns the input verbatim.
pub fn fix_mojibake(text: &str) -> String {
    match fix_mojibake_once(text) {
        Some(fixed) => fixed,
        None => text.to_string(),
    }
}

/// Count of characters that commonly appear as mojibake artifacts when UTF-8 is
/// misread as Windows-1252 (the lead bytes 0xC2/0xC3 → Â/Ã, 0xE2 → â, plus the
/// smart-punctuation soup €, ™, …, “, ”, etc.).
fn mojibake_markers(text: &str) -> usize {
    text.chars()
        .filter(|&c| {
            matches!(
                c,
                'Ã' | 'Â' | 'â' | '€' | '™' | '…' | '“' | '”' | '‘' | '’' | '\u{009D}' | '\u{0081}'
            )
        })
        .count()
}

fn fix_mojibake_once(text: &str) -> Option<String> {
    // Step 1: re-encode as Windows-1252. Reject if anything was unmappable.
    let (bytes, _, had_unmappable) = encoding_rs::WINDOWS_1252.encode(text);
    if had_unmappable {
        return None;
    }
    // Step 2: those bytes are the original UTF-8 stream — must be valid UTF-8.
    let candidate = std::str::from_utf8(&bytes).ok()?;
    // Step 3: accept only if it reduced the mojibake markers (strict improvement).
    if mojibake_markers(candidate) < mojibake_markers(text) {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Every encoding label this worker accepts, as the WHATWG/`encoding_rs` set of
/// canonical [`Encoding`] names. Used by the `supported_encodings()` table.
pub fn supported_encodings() -> Vec<&'static str> {
    // The full WHATWG Encoding Standard set that encoding_rs ships as static
    // `&'static Encoding` constants. Listed by canonical `.name()`.
    [
        encoding_rs::UTF_8,
        encoding_rs::UTF_16LE,
        encoding_rs::UTF_16BE,
        encoding_rs::IBM866,
        encoding_rs::ISO_8859_2,
        encoding_rs::ISO_8859_3,
        encoding_rs::ISO_8859_4,
        encoding_rs::ISO_8859_5,
        encoding_rs::ISO_8859_6,
        encoding_rs::ISO_8859_7,
        encoding_rs::ISO_8859_8,
        encoding_rs::ISO_8859_8_I,
        encoding_rs::ISO_8859_10,
        encoding_rs::ISO_8859_13,
        encoding_rs::ISO_8859_14,
        encoding_rs::ISO_8859_15,
        encoding_rs::ISO_8859_16,
        encoding_rs::KOI8_R,
        encoding_rs::KOI8_U,
        encoding_rs::MACINTOSH,
        encoding_rs::WINDOWS_874,
        encoding_rs::WINDOWS_1250,
        encoding_rs::WINDOWS_1251,
        encoding_rs::WINDOWS_1252,
        encoding_rs::WINDOWS_1253,
        encoding_rs::WINDOWS_1254,
        encoding_rs::WINDOWS_1255,
        encoding_rs::WINDOWS_1256,
        encoding_rs::WINDOWS_1257,
        encoding_rs::WINDOWS_1258,
        encoding_rs::X_MAC_CYRILLIC,
        encoding_rs::GBK,
        encoding_rs::GB18030,
        encoding_rs::BIG5,
        encoding_rs::EUC_JP,
        encoding_rs::ISO_2022_JP,
        encoding_rs::SHIFT_JIS,
        encoding_rs::EUC_KR,
        encoding_rs::REPLACEMENT,
        encoding_rs::X_USER_DEFINED,
    ]
    .iter()
    .map(|e| e.name())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // windows-1252 bytes for "café" — é is 0xE9 in 1252 (and in Latin-1).
    const CAFE_1252: &[u8] = &[b'c', b'a', b'f', 0xE9];

    #[test]
    fn detects_windows_1252_smart_quotes() {
        // “Hello” using 1252 curly quotes 0x93 / 0x94 around ASCII.
        let bytes = &[
            0x93, b'H', b'i', 0x94, b' ', b'w', b'o', b'r', b'l', b'd', b'!',
        ];
        let enc = detect(bytes).unwrap();
        // chardetng reports a single-byte Western encoding; windows-1252 is the
        // expected guess for these C1-range bytes.
        assert_eq!(enc, "windows-1252", "got {enc}");
    }

    #[test]
    fn detects_utf8() {
        let bytes = "Héllo, wörld — naïve façade".as_bytes();
        assert_eq!(detect(bytes), Some("UTF-8"));
    }

    #[test]
    fn detects_utf16_bom() {
        // UTF-16LE BOM (FF FE) + "Hi"
        let le = &[0xFF, 0xFE, b'H', 0x00, b'i', 0x00];
        assert_eq!(detect(le), Some("UTF-16LE"));
        // UTF-16BE BOM (FE FF) + "Hi"
        let be = &[0xFE, 0xFF, 0x00, b'H', 0x00, b'i'];
        assert_eq!(detect(be), Some("UTF-16BE"));
    }

    #[test]
    fn empty_detect_is_none() {
        assert_eq!(detect(&[]), None);
        assert_eq!(confidence(&[]), 0.0);
        assert!(to_utf8(&[]).is_none());
    }

    #[test]
    fn to_utf8_decodes_cafe() {
        assert_eq!(to_utf8(CAFE_1252).unwrap(), "café");
    }

    #[test]
    fn to_utf8_from_explicit_shift_jis() {
        // "日本語" in Shift_JIS.
        let sjis = &[0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA];
        assert_eq!(to_utf8_from(sjis, "shift_jis").unwrap(), "日本語");
    }

    #[test]
    fn to_utf8_from_unknown_label_errors() {
        assert!(to_utf8_from(b"abc", "not-a-real-encoding").is_err());
    }

    #[test]
    fn transcode_round_trip_lossless() {
        // café -> windows-1252 bytes -> back to café.
        let bytes = transcode("café", "windows-1252").unwrap();
        assert_eq!(bytes, CAFE_1252);
        assert_eq!(to_utf8_from(&bytes, "windows-1252").unwrap(), "café");
    }

    #[test]
    fn transcode_unknown_label_errors() {
        assert!(transcode("x", "nope").is_err());
    }

    #[test]
    fn confidence_lossless_is_one() {
        assert_eq!(confidence("hello".as_bytes()), 1.0);
        assert_eq!(confidence(CAFE_1252), 1.0); // decodes cleanly as 1252
    }

    #[test]
    fn fix_mojibake_cafe() {
        // "CafÃ©" is the mojibake of "Café".
        assert_eq!(fix_mojibake("CafÃ©"), "Café");
    }

    #[test]
    fn fix_mojibake_noop_on_clean_text() {
        assert_eq!(fix_mojibake("Café"), "Café");
        assert_eq!(fix_mojibake("plain ascii"), "plain ascii");
    }

    #[test]
    fn is_valid_utf8_checks() {
        assert!(is_valid_utf8("café".as_bytes()));
        assert!(!is_valid_utf8(CAFE_1252)); // 0xE9 alone is not valid UTF-8
    }

    #[test]
    fn supported_set_nonempty_and_has_utf8() {
        let set = supported_encodings();
        assert!(!set.is_empty());
        assert!(set.contains(&"UTF-8"));
        assert!(set.contains(&"Shift_JIS"));
    }
}
