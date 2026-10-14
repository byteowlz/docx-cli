//! Synthetic body frame fixtures only; no documents or text from private samples.
use super::*;
use wordcraft_doc::props::{LineSpacing, ParaProps, TextFrame};
use wordcraft_doc::{Path, Pos};

fn document(text: &str) -> Document {
    let mut d = Document::from_text(text);
    for b in &mut d.body {
        if let Block::Para(p) = Arc::make_mut(b) {
            p.props = ParaProps {
                space_before: Some(0.0),
                space_after: Some(0.0),
                line_spacing: Some(LineSpacing::Exactly(12.0)),
                widow_control: Some(false),
                ..Default::default()
            };
        }
    }
    d
}

fn set_frame(d: &mut Document, block: usize, fr: &TextFrame) {
    match Arc::make_mut(d.body.get_mut(block).unwrap()) {
        Block::Para(p) => p.props.frame = Some(fr.clone()),
        _ => panic!("frame fixture must be a paragraph"),
    }
}

fn lay(d: &Document) -> DocLayout {
    layout(d, &mut LayoutCache::new(), &LayoutOptions::default())
}

fn placed(l: &DocLayout, block: usize) -> (&ParaLayout, f32, f32, usize, usize) {
    let entries = l.index.get(&(StoryRef::Body, Path::top(block))).unwrap();
    assert_eq!(entries.len(), 1);
    let (page, item) = entries[0];
    assert_eq!(page, 0);
    match &l.pages[page].items[item] {
        Placed::Lines { para, x, y, l0, l1, turn, .. } => {
            assert_eq!(*turn, TextDirection::Horizontal);
            (para, *x, *y, *l0, *l1)
        }
        _ => panic!("expected paragraph lines"),
    }
}

fn assert_placement(l: &DocLayout, block: usize, x: f32, y: f32, width: f32, count: usize) {
    let (p, px, py, first, end) = placed(l, block);
    assert_eq!((px, py, p.lines[0].right, first, end), (x, y, width, 0, count), "block {block}");
    for line in p.lines.iter().take(end) {
        assert_eq!(line.height, 12.0);
    }
    let c = l.caret(&Pos::body(block, 0)).unwrap();
    assert_eq!(c.page, 0);
    assert_eq!(c.top, y);
}

#[test]
fn body_frame_groups_identical_paragraphs_without_consuming_flow_or_pages() {
    let mut d = document("Before\nHeading\nDetail\n\nFinal\nAfter");
    let fr = TextFrame {
        w: Some(140.0),
        h: Some(200.0),
        h_exact: true,
        x: 369.0,
        y: 175.0,
        h_anchor: "page".into(),
        v_anchor: "page".into(),
        h_space: 7.0,
        wrap: "auto".into(),
        ..Default::default()
    };
    for block in 1..=4 {
        set_frame(&mut d, block, &fr);
    }
    // In-flow layout would turn this into multiple pages.
    d.last_section.page_h = 160.0;
    d.last_section.margin_top = 20.0;
    d.last_section.margin_bottom = 20.0;
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    let w = d.last_section.text_width();
    assert_placement(&l, 0, 72.0, 20.0, w, 1);
    for block in 1..=4 {
        assert_placement(&l, block, 369.0, 175.0 + (block - 1) as f32 * 12.0, 140.0, 1);
    }
    assert_placement(&l, 5, 72.0, 32.0, w, 1);
    assert_eq!(l.index.len(), 6);
}

#[test]
fn body_frame_anchor_offsets_and_alignments_use_page_margin_and_column_areas() {
    for (h, v, x, y) in [("page", "page", 15.0, 24.0), ("margin", "margin", 87.0, 96.0), ("text", "text", 87.0, 108.0)] {
        let mut d = document("Before\nFrame\nAfter");
        let fr = TextFrame {
            w: Some(100.0),
            h: Some(40.0),
            x: 15.0,
            y: 24.0,
            h_anchor: h.into(),
            v_anchor: v.into(),
            wrap: "none".into(),
            ..Default::default()
        };
        set_frame(&mut d, 1, &fr);
        let l = lay(&d);
        assert_eq!(l.pages.len(), 1);
        assert_placement(&l, 1, x, y, 100.0, 1);
        assert_placement(&l, 2, 72.0, 84.0, d.last_section.text_width(), 1);
    }
    let mut d = document("Frame\nAfter");
    let fr = TextFrame {
        w: Some(100.0),
        h: Some(40.0),
        h_exact: true,
        x: 999.0,
        y: 999.0,
        x_align: Some("center".into()),
        y_align: Some("bottom".into()),
        h_anchor: "margin".into(),
        v_anchor: "page".into(),
        wrap: "none".into(),
        ..Default::default()
    };
    set_frame(&mut d, 0, &fr);
    let l = lay(&d);
    assert_placement(&l, 0, 72.0 + (d.last_section.text_width() - 100.0) / 2.0, d.last_section.page_h - 40.0, 100.0, 1);
    assert_placement(&l, 1, 72.0, 72.0, d.last_section.text_width(), 1);
}

#[test]
fn body_frame_height_rules_and_wrap_distances_use_group_bounds() {
    for (h, exact, bottom) in [(Some(60.0), true, 132.0), (Some(10.0), false, 108.0), (None, false, 108.0)] {
        let mut d = document("First\nSecond\nThird\nAfter");
        let fr = TextFrame {
            w: Some(100.0),
            h,
            h_exact: exact,
            h_anchor: "margin".into(),
            v_anchor: "margin".into(),
            wrap: "notBeside".into(),
            v_space: 5.0,
            ..Default::default()
        };
        for block in 0..3 {
            set_frame(&mut d, block, &fr);
        }
        let l = lay(&d);
        assert_eq!(l.pages.len(), 1);
        for block in 0..3 {
            assert_placement(&l, block, 72.0, 72.0 + block as f32 * 12.0, 100.0, 1);
        }
        assert_placement(&l, 3, 72.0, bottom + 5.0, d.last_section.text_width(), 1);
    }
    for wrap in ["around", "auto", "tight", "through", "none"] {
        let mut d = document("Frame\nText beside a frame.");
        let fr = TextFrame {
            w: Some(100.0),
            h: Some(60.0),
            h_exact: true,
            h_anchor: "text".into(),
            v_anchor: "text".into(),
            wrap: wrap.into(),
            h_space: 7.0,
            ..Default::default()
        };
        set_frame(&mut d, 0, &fr);
        let l = lay(&d);
        assert_placement(&l, 0, 72.0, 72.0, 100.0, 1);
        assert_placement(&l, 1, 72.0, 72.0, d.last_section.text_width(), 1);
        let (p, _, _, _, _) = placed(&l, 1);
        assert_eq!(p.lines[0].left, if wrap == "none" { 0.0 } else { 107.0 });
        assert_eq!(l.caret(&Pos::body(1, 0)).unwrap().x, if wrap == "none" { 72.0 } else { 179.0 });
    }
}

#[test]
fn body_frame_explicit_width_rebreaks_text_and_exact_height_hides_overflow() {
    let mut d = document("one two three four five six seven eight nine ten\nAfter");
    let fr = TextFrame {
        w: Some(60.0),
        h: Some(24.0),
        h_exact: true,
        x: 300.0,
        y: 200.0,
        h_anchor: "page".into(),
        v_anchor: "page".into(),
        wrap: "none".into(),
        ..Default::default()
    };
    set_frame(&mut d, 0, &fr);
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    assert_placement(&l, 0, 300.0, 200.0, 60.0, 2);
    assert!(placed(&l, 0).0.lines.len() > 2);
    assert_placement(&l, 1, 72.0, 72.0, d.last_section.text_width(), 1);
}

#[test]
fn body_frame_full_group_removes_the_extra_page() {
    let text = std::iter::repeat_n("Frame", 18).chain(std::iter::repeat_n("Flow", 45)).collect::<Vec<_>>().join("\n");
    let mut d = document(&text);
    assert_eq!(lay(&d).pages.len(), 2);
    let fr = TextFrame {
        w: Some(140.0),
        h: Some(240.0),
        h_exact: true,
        x: 369.0,
        y: 175.0,
        h_anchor: "page".into(),
        v_anchor: "page".into(),
        wrap: "auto".into(),
        ..Default::default()
    };
    for block in 0..18 {
        set_frame(&mut d, block, &fr);
    }
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    assert_eq!(l.index.len(), 63);
    for block in 0..18 {
        assert_placement(&l, block, 369.0, 175.0 + block as f32 * 12.0, 140.0, 1);
    }
    for block in 18..63 {
        let y = 72.0 + (block - 18) as f32 * 12.0;
        let right = if y + 12.0 > 175.0 && y < 415.0 { 297.0 } else { d.last_section.text_width() };
        assert_placement(&l, block, 72.0, y, right, 1);
    }
}

#[test]
fn body_frame_auto_width_uses_widest_group_member_and_alignment_has_no_padding() {
    for width in [None, Some(0.0)] {
        let mut d = document("X\nWidest member\nAfter");
        let fr = TextFrame {
            w: width,
            x_align: Some("right".into()),
            h_anchor: "text".into(),
            v_anchor: "text".into(),
            wrap: "none".into(),
            ..Default::default()
        };
        let plain = lay(&d);
        let pl = placed(&plain, 1).0;
        let expected_w = natural_width(&Arc::new(pl.clone()));
        for block in 0..2 {
            set_frame(&mut d, block, &fr);
            d.para_mut(StoryRef::Body, &Path::top(block)).unwrap().props.align = Some(wordcraft_doc::props::Align::Center);
        }
        let l = lay(&d);
        let x = 72.0 + d.last_section.text_width() - expected_w;
        for block in 0..2 {
            let (p, px, py, a, b) = placed(&l, block);
            assert!((px - x).abs() < 0.001);
            assert_eq!((py, a, b), (72.0 + block as f32 * 12.0, 0, 1));
            assert!((p.lines[0].right - expected_w).abs() < 0.001);
            let line = &p.lines[0];
            let ink = p.x_of(0, line.stop).unwrap() - p.x_of(0, line.start).unwrap();
            assert!((p.x_of(0, line.start).unwrap() - (expected_w - ink) / 2.0).abs() < 0.001);
        }
        assert_placement(&l, 2, 72.0, 72.0, d.last_section.text_width(), 1);
    }
}

#[test]
fn body_frame_follows_explicit_anchor_page_break_and_inside_outside_page_parity() {
    for (alignment, expected_x) in [("inside", 512.0), ("outside", 0.0)] {
        let mut d = document("Before\nFrame\nAfter");
        let fr = TextFrame {
            w: Some(100.0),
            y: 30.0,
            x_align: Some(alignment.into()),
            h_anchor: "page".into(),
            v_anchor: "page".into(),
            wrap: "none".into(),
            ..Default::default()
        };
        set_frame(&mut d, 1, &fr);
        d.para_mut(StoryRef::Body, &Path::top(2)).unwrap().props.page_break_before = Some(true);
        let l = lay(&d);
        assert_eq!(l.pages.len(), 2);
        assert_eq!(l.index[&(StoryRef::Body, Path::top(0))][0].0, 0);
        for block in 1..3 {
            assert_eq!(l.index[&(StoryRef::Body, Path::top(block))][0].0, 1);
        }
        let (pi, ii) = l.index[&(StoryRef::Body, Path::top(1))][0];
        match &l.pages[pi].items[ii] {
            Placed::Lines { x, y, l0, l1, para, .. } => assert_eq!((*x, *y, *l0, *l1, para.lines[0].right), (expected_x, 30.0, 0, 1, 100.0)),
            _ => panic!("frame lines"),
        }
    }
}

#[test]
fn body_frame_section_boundary_ends_a_group_even_with_identical_properties() {
    let mut d = document("First\nSecond\nFlow");
    let fr =
        TextFrame { w: Some(100.0), x: 300.0, y: 200.0, h_anchor: "page".into(), v_anchor: "page".into(), wrap: "none".into(), ..Default::default() };
    for block in 0..2 {
        set_frame(&mut d, block, &fr);
    }
    d.para_mut(StoryRef::Body, &Path::top(0)).unwrap().section = Some(Box::new(d.last_section.clone()));
    d.last_section.start = SectionStart::Continuous;
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    assert_placement(&l, 0, 300.0, 200.0, 100.0, 1);
    assert_placement(&l, 1, 300.0, 200.0, 100.0, 1);
    assert_placement(&l, 2, 72.0, 72.0, d.last_section.text_width(), 1);
}

#[test]
fn body_frame_reflow_views_and_explicit_inline_alignment_keep_reading_order() {
    let mut d = document("Frame\nFlow");
    let fr =
        TextFrame { w: Some(100.0), x: 300.0, y: 200.0, h_anchor: "page".into(), v_anchor: "page".into(), wrap: "none".into(), ..Default::default() };
    set_frame(&mut d, 0, &fr);
    for view in [ViewMode::Web, ViewMode::Draft, ViewMode::Read] {
        let opts = LayoutOptions { view, ..Default::default() };
        let l = layout(&d, &mut LayoutCache::new(), &opts);
        let a = l.caret(&Pos::body(0, 0)).unwrap();
        let b = l.caret(&Pos::body(1, 0)).unwrap();
        assert_eq!((a.page, b.page, a.x, b.x, b.top - a.top), (0, 0, 18.0, 18.0, 12.0));
    }
    set_frame(&mut d, 0, &TextFrame { y_align: Some("inline".into()), ..fr });
    let l = lay(&d);
    assert_placement(&l, 0, 72.0, 72.0, d.last_section.text_width(), 1);
    assert_placement(&l, 1, 72.0, 84.0, d.last_section.text_width(), 1);
}

#[test]
fn body_frame_spacing_is_local_and_inherited_frames_group() {
    use wordcraft_doc::styles::{Style, StyleKind};
    let mut d = document("Before\nFirst\nSecond\nAfter");
    let fr =
        TextFrame { w: Some(100.0), x: 300.0, y: 200.0, h_anchor: "page".into(), v_anchor: "page".into(), wrap: "none".into(), ..Default::default() };
    d.styles.upsert(Style {
        id: "FrameStyle".into(),
        name: "Frame Style".into(),
        kind: StyleKind::Paragraph,
        para: ParaProps { frame: Some(fr), ..Default::default() },
        ..Default::default()
    });
    d.para_mut(StoryRef::Body, &Path::top(0)).unwrap().props.space_after = Some(10.0);
    for block in 1..3 {
        let p = d.para_mut(StoryRef::Body, &Path::top(block)).unwrap();
        p.props.style = Some("FrameStyle".into());
        p.props.space_before = Some(6.0);
        p.props.space_after = Some(8.0);
    }
    d.para_mut(StoryRef::Body, &Path::top(3)).unwrap().props.space_before = Some(15.0);
    let l = lay(&d);
    assert_placement(&l, 1, 300.0, 206.0, 100.0, 1);
    assert_placement(&l, 2, 300.0, 226.0, 100.0, 1);
    assert_placement(&l, 3, 72.0, 99.0, d.last_section.text_width(), 1);
}

#[test]
fn body_frame_group_ends_on_property_change_and_normal_paragraph() {
    let mut d = document("First\nChanged\nFlow\nSeparate\nAfter");
    let fr =
        TextFrame { w: Some(100.0), x: 300.0, y: 200.0, h_anchor: "page".into(), v_anchor: "page".into(), wrap: "none".into(), ..Default::default() };
    set_frame(&mut d, 0, &fr);
    set_frame(&mut d, 1, &TextFrame { x: 420.0, ..fr.clone() });
    set_frame(&mut d, 3, &fr);
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    assert_placement(&l, 0, 300.0, 200.0, 100.0, 1);
    assert_placement(&l, 1, 420.0, 200.0, 100.0, 1);
    assert_placement(&l, 2, 72.0, 72.0, d.last_section.text_width(), 1);
    assert_placement(&l, 3, 300.0, 200.0, 100.0, 1);
    assert_placement(&l, 4, 72.0, 84.0, d.last_section.text_width(), 1);
}
