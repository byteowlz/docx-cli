//! Floating body paragraph frames (`w:framePr`). Consecutive paragraphs with identical
//! resolved frame properties are one box, not independent overlapping floats.
use super::*;
use wordcraft_doc::props::TextFrame;

/// Handle a framed body paragraph, including already placed members of a group. Reflow views
/// keep the ordinary reading order. A section boundary always ends a frame group.
pub(super) fn place(ctx: &mut Ctx, pb: &mut PageBuilder, p: &Paragraph, block: usize, body_top: f32) -> bool {
    if pb.web || ctx.opts.view == ViewMode::Read {
        return false;
    }
    let rp = ctx.doc.styles.resolve_para(&p.props);
    let Some(fr) = rp.frame else { return false };
    // `inline` is explicitly in the flow; a frame with an offset (including zero) floats.
    if fr.y_align.as_deref() == Some("inline") {
        return false;
    }
    if block
        .checked_sub(1)
        .and_then(|i| ctx.doc.body.get(i))
        .and_then(|b| b.as_para())
        .is_some_and(|prev| prev.section.is_none() && ctx.doc.styles.resolve_para(&prev.props).frame.as_ref() == Some(&fr))
    {
        return true;
    }
    if rp.page_break_before && !pb.at_top() {
        pb.new_page(block, body_top);
    }
    ctx.fields.page = pb.number;
    let doc = ctx.doc;
    let mut end = block;
    while let Some(p) = doc.body.get(end).and_then(|b| b.as_para()) {
        if doc.styles.resolve_para(&p.props).frame.as_ref() != Some(&fr) {
            break;
        }
        end = end.saturating_add(1);
        if p.section.is_some() {
            break;
        }
    }
    // A frame is anchored to the next non-frame paragraph, including an explicit break
    // before that paragraph. Move the frame with its anchor rather than leaving it behind.
    if anchor_breaks_page(doc, end) && !pb.at_top() {
        pb.new_page(block, body_top);
        ctx.fields.page = pb.number;
    }
    let (mut items, w, h) = contents(ctx, block..end, &fr, pb.col_w());
    let mut float = frame_float(&fr);
    if pb.number.is_multiple_of(2) {
        float.h_align = match float.h_align {
            Some(FloatAlign::Inside) => Some(FloatAlign::End),
            Some(FloatAlign::Outside) => Some(FloatAlign::Start),
            other => other,
        };
    }
    let r = float_rect(Some(PageFrame { sect: pb.sect, origin: (0.0, 0.0) }), (pb.col_x(), pb.col_w()), pb.y, w, h, &float);
    for it in &mut items {
        it.translate(r.x, r.y);
    }
    table_notes(ctx, pb, &items);
    // Floating tables must not overlap a wrapping text frame either. A no-wrap frame imposes
    // no exclusion (and must not push a later floating table).
    if let Some(area) = wrap_area(r, &float, None) {
        pb.float_tables.push(area.0);
        pb.excl.push(area);
    }
    if let Some(pg) = pb.page() {
        pg.items.extend(items);
    }
    // Deliberately leave y, previous-flow spacing and column balancing height untouched.
    true
}

/// Do not look for an anchor across a table or section boundary.
fn anchor_breaks_page(doc: &Document, end: usize) -> bool {
    if end.checked_sub(1).and_then(|i| doc.body.get(i)).and_then(|b| b.as_para()).is_some_and(|p| p.section.is_some()) {
        return false;
    }
    for b in doc.body.iter().skip(end) {
        let Some(p) = b.as_para() else { break };
        let rp = doc.styles.resolve_para(&p.props);
        if rp.frame.is_none() {
            return rp.page_break_before;
        }
        if p.section.is_some() {
            break;
        }
    }
    false
}

/// Collapse paragraph spacing inside the frame independently of the surrounding body flow.
fn spacing_before(rp: &wordcraft_doc::resolve::ResolvedPara, listed: bool, prev: Option<(String, bool, f32, bool, bool)>, y: &mut f32) -> f32 {
    let mut before = rp.space_before;
    let Some((style, contextual, mut after, auto, prev_listed)) = prev else {
        return if rp.before_auto { 0.0 } else { before };
    };
    if style == rp.style {
        if contextual {
            *y -= after;
            after = 0.0;
        }
        if rp.contextual_spacing {
            before = 0.0;
        }
    }
    if listed && prev_listed {
        if auto {
            *y -= after;
            after = 0.0;
        }
        if rp.before_auto {
            before = 0.0;
        }
    }
    if before > 0.0 && after > 0.0 { (before - after).max(0.0) } else { before }
}

/// Lay out a frame's paragraphs at their shared width, retaining their original body paths
/// (caret, selection, semantic inspection and MCP still address the same document).
fn contents(ctx: &mut Ctx, blocks: std::ops::Range<usize>, fr: &TextFrame, column_width: f32) -> (Vec<Placed>, f32, f32) {
    let doc = ctx.doc;
    let paras: Vec<_> = blocks.filter_map(|i| doc.body.get(i).and_then(|b| b.as_para()).map(|p| (i, p, ctx.next_label(p)))).collect();
    let w = fr
        .w
        .filter(|w| w.is_finite() && *w > 0.0)
        .unwrap_or_else(|| {
            // Measure without alignment padding; zero/omitted width fits the widest paragraph, not
            // the first one. Measuring must not count equations a second time when rebreaking.
            let eq = ctx.eq_count;
            let widest =
                paras.iter().map(|(_, p, label)| natural_width(&ctx.para_labelled(p, column_width, None, &[], label.clone()))).fold(1.0, f32::max);
            ctx.eq_count = eq;
            widest
        })
        .clamp(1.0, 1584.0);
    let mut items = Vec::new();
    let mut y = 0.0;
    let mut prev: Option<(String, bool, f32, bool, bool)> = None;
    let mut exclusions = Vec::new();
    for (k, (block, p, label)) in paras.iter().enumerate() {
        let mut pl = ctx.para_labelled(p, w, None, &[], label.clone());
        let anchor_y = y;
        let before = spacing_before(&pl.rp, label.is_some(), prev.take(), &mut y);
        y += before;
        let mut floats = HashMap::new();
        let left_out = left_out_objects(doc, p, None, ctx.opts.show_hidden, ctx.hide_deleted);
        for (oi, o) in p.objects.iter().enumerate() {
            let Some((ow, oh, f)) = floating(o).filter(|_| !left_out(oi)) else { continue };
            let r = float_rect(None, (0.0, w), anchor_y, ow, oh, f);
            let outline = ctx.wrap_outline(o);
            exclusions.extend(wrap_area(r, f, outline.as_deref().map(|v| v.as_slice())));
            floats.insert(oi, r);
        }
        let rel = rel_exclusions(&exclusions, 0.0, y);
        if !rel.is_empty() {
            pl = ctx.para_labelled(p, w, None, &rel, label.clone());
        }
        let lead = pl.lines.first().map_or(0.0, |l| l.top.max(0.0));
        let path = [*block as u32];
        push_para(&mut items, StoryRef::Body, &path, &pl, 0, pl.lines.len(), 0.0, y + lead, w);
        let at = ObjFrame { page: None, col: (0.0, w), para_y: anchor_y, table_chr: None };
        let (behind, front) = place_objects(ctx, StoryRef::Body, &path, p, &pl, (0, pl.lines.len()), (0.0, y + lead), &at, &floats, 0);
        items.splice(0..0, behind);
        items.extend(front);
        let after = if pl.rp.after_auto && k + 1 == paras.len() { 0.0 } else { pl.rp.space_after };
        y += pl.height + after;
        prev = Some((pl.rp.style.clone(), pl.rp.contextual_spacing, after, pl.rp.after_auto, label.is_some()));
    }
    let h = fr.h.filter(|h| h.is_finite() && *h > 0.0).map_or(y, |h| if fr.h_exact { h } else { h.max(y) }).clamp(1.0, 4000.0);
    if fr.h_exact && h < y {
        items = fit_box(items, h);
    }
    (items, w, h)
}
