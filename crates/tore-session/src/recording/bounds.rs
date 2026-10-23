//! Shared recorder bounds, preserving feedback before ordinary events.
use tore_replay::{self as replay, Event, Frame, vocab::kind};
pub fn fit(frame: &mut Frame) {
    use tore_replay::limits::*;
    fn cut<T>(list: &mut Vec<T>, limit: usize, what: &str, notes: &mut Vec<String>) {
        if list.len() > limit {
            notes.push(format!("{} {what}", list.len() - limit));
            list.truncate(limit);
        }
    }
    fn short(text: &mut String) {
        if text.len() > MAX_STRING_BYTES {
            let mut end = MAX_STRING_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
    }
    let mut notes = Vec::new();
    cut(&mut frame.aircraft, MAX_AIRCRAFT, "aircraft", &mut notes);
    cut(
        &mut frame.projectiles,
        MAX_PROJECTILES,
        "projectiles",
        &mut notes,
    );
    cut(&mut frame.debris, MAX_DEBRIS, "debris pieces", &mut notes);
    cut(
        &mut frame.escapees,
        MAX_ESCAPEES,
        "ejected pilots",
        &mut notes,
    );
    cut(
        &mut frame.new_effects,
        MAX_EFFECTS_PER_TICK,
        "effects",
        &mut notes,
    );
    cut(
        &mut frame.new_puffs,
        MAX_PUFFS_PER_TICK,
        "smoke puffs",
        &mut notes,
    );
    cut(
        &mut frame.surface_hp,
        MAX_SURFACE_CHANGES_PER_TICK,
        "surface changes",
        &mut notes,
    );
    cut(
        &mut frame.surface,
        MAX_SURFACE_UNITS,
        "surface units",
        &mut notes,
    );
    cut(
        &mut frame.surface_stock,
        MAX_SURFACE_STOCK_PER_TICK,
        "surface stock changes",
        &mut notes,
    );
    cut(
        &mut frame.debris_pieces,
        MAX_DEBRIS,
        "debris pieces",
        &mut notes,
    );
    for event in &mut frame.events {
        short(&mut event.text);
        event.fields.truncate(MAX_FIELDS_PER_EVENT);
        for (_, value) in &mut event.fields {
            match value {
                replay::Value::Text(text) => short(text),
                replay::Value::Ids(ids) => ids.truncate(MAX_IDS_PER_VALUE),
                _ => {}
            }
        }
    }
    if frame.events.len() >= MAX_EVENTS_PER_TICK {
        frame
            .events
            .sort_by_key(|e| e.kind != replay::feedback::KIND);
    }
    cut(
        &mut frame.events,
        MAX_EVENTS_PER_TICK - 1,
        "events",
        &mut notes,
    );
    cut(
        &mut frame.trees,
        MAX_TREES_PER_TICK,
        "display trees",
        &mut notes,
    );
    for tree in &mut frame.trees {
        if tree.nodes.len() > MAX_TREE_NODES {
            notes.push(format!(
                "{} nodes of a {} tree",
                tree.nodes.len() - MAX_TREE_NODES,
                tree.channel
            ));
            tree.nodes.truncate(MAX_TREE_NODES);
        }
        for node in &mut tree.nodes {
            node.depth = node.depth.min(MAX_TREE_DEPTH - 1);
            short(&mut node.label);
            short(&mut node.note);
            short(&mut node.unit);
            match &mut node.value {
                replay::Value::Text(text) => short(text),
                replay::Value::Ids(ids) => ids.truncate(MAX_IDS_PER_VALUE),
                _ => {}
            }
        }
    }
    if !notes.is_empty() {
        frame
            .events
            .push(Event::new(kind::SYSTEM_NOTE).with_text(format!(
                "over the format's limits, left out: {}",
                notes.join(", ")
            )));
    }
}
