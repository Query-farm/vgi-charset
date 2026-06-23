//! Integration tests against the pure engine with KNOWN byte sequences — these
//! exercise `charset-worker`'s library surface without Arrow or RPC.

use charset_worker::charset;

// windows-1252 bytes for "café": é is 0xE9.
const CAFE_1252: &[u8] = &[b'c', b'a', b'f', 0xE9];
// "日本語" in Shift_JIS.
const NIHONGO_SJIS: &[u8] = &[0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA];

#[test]
fn detect_windows_1252() {
    // 1252 smart quotes 0x93/0x94 around ASCII.
    let bytes = &[0x93, b'q', b'u', b'o', b't', b'e', 0x94];
    assert_eq!(charset::detect(bytes), Some("windows-1252"));
}

#[test]
fn detect_valid_utf8() {
    assert_eq!(
        charset::detect("a UTF-8 façade — café".as_bytes()),
        Some("UTF-8")
    );
}

#[test]
fn detect_utf16_bom_le_and_be() {
    assert_eq!(
        charset::detect(&[0xFF, 0xFE, b'H', 0x00, b'i', 0x00]),
        Some("UTF-16LE")
    );
    assert_eq!(
        charset::detect(&[0xFE, 0xFF, 0x00, b'H', 0x00, b'i']),
        Some("UTF-16BE")
    );
}

#[test]
fn to_utf8_of_windows_1252_cafe() {
    assert_eq!(charset::to_utf8(CAFE_1252).unwrap(), "café");
}

#[test]
fn to_utf8_from_shift_jis() {
    assert_eq!(
        charset::to_utf8_from(NIHONGO_SJIS, "shift_jis").unwrap(),
        "日本語"
    );
}

#[test]
fn to_utf8_from_unknown_label_is_err() {
    assert!(charset::to_utf8_from(b"abc", "totally-bogus").is_err());
}

#[test]
fn transcode_round_trip_lossless() {
    // Round-trip: decode the 1252 bytes to UTF-8, re-encode to 1252, expect the
    // original bytes back (lossless for this content).
    let utf8 = charset::to_utf8(CAFE_1252).unwrap();
    let back = charset::transcode(&utf8, "windows-1252").unwrap();
    assert_eq!(back, CAFE_1252);
}

#[test]
fn transcode_shift_jis_round_trip() {
    let utf8 = charset::to_utf8_from(NIHONGO_SJIS, "shift_jis").unwrap();
    let back = charset::transcode(&utf8, "shift_jis").unwrap();
    assert_eq!(back, NIHONGO_SJIS);
}

#[test]
fn fix_mojibake_cafe() {
    assert_eq!(charset::fix_mojibake("CafÃ©"), "Café");
}

#[test]
fn fix_mojibake_smart_quotes() {
    // "â€œHiâ€\u{9d}" is the mojibake of curly-quoted "“Hi”".
    assert_eq!(charset::fix_mojibake("â€œHiâ€\u{9d}"), "“Hi”");
}

#[test]
fn fix_mojibake_noops_on_clean() {
    assert_eq!(charset::fix_mojibake("Café"), "Café");
    assert_eq!(charset::fix_mojibake("ascii only"), "ascii only");
}

#[test]
fn is_valid_utf8_known() {
    assert!(charset::is_valid_utf8("café".as_bytes()));
    assert!(!charset::is_valid_utf8(CAFE_1252));
}

#[test]
fn empty_and_confidence() {
    assert_eq!(charset::detect(&[]), None);
    assert!(charset::to_utf8(&[]).is_none());
    assert_eq!(charset::confidence(&[]), 0.0);
    assert_eq!(charset::confidence("plain".as_bytes()), 1.0);
}

#[test]
fn supported_encodings_nonempty() {
    let set = charset::supported_encodings();
    assert!(set.len() > 1);
    assert!(set.contains(&"UTF-8"));
}
