use super::*;

#[test]
fn native_json_round_trip_preserves_full_document() {
    let mut doc = Document::from_text("Synthetic logo document\nSecond paragraph");
    doc.format_range(&Pos::body(0, 0), &Pos::body(0, 9), &|c| c.bold = Some(true)).unwrap();
    doc.add_part(PartKind::Header, vec![para_block(Paragraph::with_text("Synthetic header", CharProps::default()))]);
    let media = doc.add_media((0..=255).collect(), "png");
    doc.insert_object(
        &Pos::body(1, 0),
        InlineObject::Image { media, w: 36.0, h: 24.0, alt: "Synthetic logo".into(), float: Default::default(), crop: [0.0; 4], ole: None },
        &CharProps::default(),
    )
    .unwrap();
    doc.media.insert("empty.bin".into(), Arc::new(Vec::new()));
    doc.passthrough.insert("customXml/item1.xml".into(), Arc::new(b"<synthetic>opaque data</synthetic>".to_vec()));
    doc.passthrough.insert("word/embeddings/synthetic.bin".into(), Arc::new(vec![0, 255, 128, 13, 10]));
    doc.passthrough.insert("empty.bin".into(), Arc::new(Vec::new()));

    let mut json = Vec::new();
    serde_json::to_writer_pretty(&mut json, &doc).unwrap();
    let mut restored: Document = serde_json::from_reader(json.as_slice()).unwrap();
    assert_eq!(restored, doc);
    // The engine calls this repair after opening native JSON; opaque bytes must be untouched.
    restored.ensure_nonempty();
    assert_eq!(restored, doc);
    assert_eq!(serde_json::to_vec_pretty(&restored).unwrap(), json);

    let compact = serde_json::to_string(&doc).unwrap();
    let restored: Document = serde_json::from_str(&compact).unwrap();
    assert_eq!(restored, doc);
    assert_eq!(serde_json::to_string(&restored).unwrap(), compact);
}

#[test]
fn native_json_reads_byte_maps_in_deterministic_key_order() {
    // Deliberately reverse input order, with escaped and non-ASCII keys and non-UTF-8 bytes.
    let json = r#"{"media":{"z.png":[255,0],"a\"é.png":[128,1]},"passthrough":{"z.bin":[],"a.xml":[60,62]}}"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    let mut expected = Document::new();
    expected.media.insert("z.png".into(), Arc::new(vec![255, 0]));
    expected.media.insert("a\"é.png".into(), Arc::new(vec![128, 1]));
    expected.passthrough.insert("z.bin".into(), Arc::new(Vec::new()));
    expected.passthrough.insert("a.xml".into(), Arc::new(vec![60, 62]));
    assert_eq!(doc, expected);

    let written = serde_json::to_string(&doc).unwrap();
    assert!(written.contains(r#""media":{"a\"é.png":[128,1],"z.png":[255,0]}"#));
    assert!(written.contains(r#""passthrough":{"a.xml":[60,62],"z.bin":[]}"#));
    let value: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert!(value.get("body").is_some(), "native Document remains the root object");
    assert_eq!(serde_json::from_str::<Document>(&written).unwrap(), expected);
}

#[test]
fn native_json_empty_maps_are_omitted_and_missing_maps_default() {
    let doc = Document::new();
    let value = serde_json::to_value(&doc).unwrap();
    assert!(value.get("media").is_none());
    assert!(value.get("passthrough").is_none());
    assert_eq!(serde_json::from_value::<Document>(value).unwrap(), doc);
    assert_eq!(serde_json::from_str::<Document>("{}").unwrap(), doc);
    assert_eq!(serde_json::from_str::<Document>(r#"{"media":{},"passthrough":{}}"#).unwrap(), doc);
}

#[test]
fn native_json_rejects_invalid_byte_maps() {
    for field in ["media", "passthrough"] {
        for payload in ["[-1]", "[256]", "[1.5]", "[null]", r#"["byte"]"#, r#""not an array""#, "null"] {
            let json = format!(r#"{{"{field}":{{"synthetic.bin":{payload}}}}}"#);
            assert!(serde_json::from_str::<Document>(&json).is_err(), "accepted {json}");
        }
    }
}
