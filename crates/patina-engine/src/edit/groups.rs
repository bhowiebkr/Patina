//! The groups and the notes of the canvas (spec 15.12). The build does not read them.

use crate::project::{GraphFile, Group, Note, is_id};

/// The smallest size of a group, in graph pixels.
const MIN_SIZE: [f64; 2] = [120.0, 60.0];
const MAX_TITLE: usize = 64;
const MAX_TEXT: usize = 20_000;

/// True when a node, a group or a note of the graph has the ID (6.4).
pub(crate) fn used(graph: &GraphFile, id: &str) -> bool {
    graph.node.iter().any(|n| n.id == id) || graph.group.iter().any(|g| g.id == id) || graph.note.iter().any(|n| n.id == id)
}

/// Checks the limits of a group (6.4): the ID, the title and a finite size of 120 x 60 at least.
pub(crate) fn check_group(group: &Group) -> Result<(), String> {
    check_id(&group.id)?;
    if group.title.chars().count() > MAX_TITLE || group.title.chars().any(char::is_control) {
        return Err(format!("a group title has {MAX_TITLE} characters at most and no control characters"));
    }
    if !group.pos.iter().chain(&group.size).all(|v| v.is_finite()) {
        return Err("the position or the size of the group is not a finite number".to_owned());
    }
    if group.size[0] < MIN_SIZE[0] || group.size[1] < MIN_SIZE[1] {
        return Err(format!("a group is {} x {} at least", MIN_SIZE[0], MIN_SIZE[1]));
    }
    Ok(())
}

/// Checks the limits of a note (6.4): the ID, the text and a width from 80 to 2000.
pub(crate) fn check_note(note: &Note) -> Result<(), String> {
    check_id(&note.id)?;
    if note.text.chars().count() > MAX_TEXT || note.text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(format!("a note has {MAX_TEXT} characters at most, and only line feeds and tabs as control characters"));
    }
    if !note.pos.iter().all(|v| v.is_finite()) {
        return Err("the position of the note is not a finite number".to_owned());
    }
    if !(80.0..=2000.0).contains(&note.width) {
        return Err("a note is 80 to 2000 pixels wide".to_owned());
    }
    Ok(())
}

/// Adds the group, or replaces the group with the same ID.
pub(crate) fn set_group(graph: &mut GraphFile, group: &Group) -> Result<(), String> {
    check_group(group)?;
    match graph.group.iter().position(|g| g.id == group.id) {
        Some(i) => graph.group[i] = group.clone(),
        None if used(graph, &group.id) => return Err(format!("the graph has an item with the ID {}", group.id)),
        None => graph.group.push(group.clone()),
    }
    Ok(())
}

/// Adds the note, or replaces the note with the same ID.
pub(crate) fn set_note(graph: &mut GraphFile, note: &Note) -> Result<(), String> {
    check_note(note)?;
    match graph.note.iter().position(|n| n.id == note.id) {
        Some(i) => graph.note[i] = note.clone(),
        None if used(graph, &note.id) => return Err(format!("the graph has an item with the ID {}", note.id)),
        None => graph.note.push(note.clone()),
    }
    Ok(())
}

pub(crate) fn remove_group(graph: &mut GraphFile, id: &str) -> Result<(), String> {
    let count = graph.group.len();
    graph.group.retain(|g| g.id != id);
    if graph.group.len() == count { Err(format!("the graph has no group {id}")) } else { Ok(()) }
}

pub(crate) fn remove_note(graph: &mut GraphFile, id: &str) -> Result<(), String> {
    let count = graph.note.len();
    graph.note.retain(|n| n.id != id);
    if graph.note.len() == count { Err(format!("the graph has no note {id}")) } else { Ok(()) }
}

fn check_id(id: &str) -> Result<(), String> {
    if is_id(id) { Ok(()) } else { Err(format!("the ID {id:?} is not 16 lowercase hexadecimal characters")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::GroupColor;

    const G: &str = "00000000000000g1";

    fn group(id: &str) -> Group {
        Group { id: id.to_owned(), title: "Color Grade".to_owned(), color: GroupColor::Amber, pos: [0.0, 0.0], size: [320.0, 200.0] }
    }

    fn note(id: &str) -> Note {
        Note { id: id.to_owned(), pos: [0.0, 0.0], width: 320.0, text: "## Notes\n- one\tand two".to_owned() }
    }

    #[test]
    fn a_set_adds_or_replaces_by_id_and_a_remove_needs_the_item() {
        let id = "00000000000000a1";
        let mut graph = GraphFile::default();
        set_group(&mut graph, &group(id)).unwrap();
        let mut bigger = group(id);
        bigger.size = [400.0, 300.0];
        set_group(&mut graph, &bigger).unwrap();
        assert_eq!(graph.group, vec![bigger]);
        assert!(set_note(&mut graph, &note(id)).is_err(), "a note can not take the ID of a group");
        set_note(&mut graph, &note("00000000000000b1")).unwrap();
        remove_group(&mut graph, id).unwrap();
        assert!(remove_group(&mut graph, id).is_err());
        remove_note(&mut graph, "00000000000000b1").unwrap();
        assert_eq!(graph, GraphFile::default());
    }

    #[test]
    fn the_limits_refuse_bad_groups_and_notes() {
        assert!(check_group(&group(G)).is_err(), "g is not hexadecimal");
        let id = "00000000000000a1";
        let bad_groups = [
            Group { title: "x".repeat(65), ..group(id) },
            Group { title: "a\nb".to_owned(), ..group(id) },
            Group { size: [119.0, 200.0], ..group(id) },
            Group { size: [320.0, f64::NAN], ..group(id) },
            Group { pos: [f64::INFINITY, 0.0], ..group(id) },
        ];
        for g in &bad_groups {
            assert!(check_group(g).is_err(), "{g:?}");
        }
        check_group(&Group { title: "x".repeat(64), size: [120.0, 60.0], ..group(id) }).unwrap();
        let bad_notes = [
            Note { text: "x".repeat(20_001), ..note(id) },
            Note { text: "bell\u{7}".to_owned(), ..note(id) },
            Note { width: 79.0, ..note(id) },
            Note { width: f64::NAN, ..note(id) },
            Note { pos: [0.0, f64::NAN], ..note(id) },
        ];
        for n in &bad_notes {
            assert!(check_note(n).is_err(), "{n:?}");
        }
        check_note(&Note { text: String::new(), width: 2000.0, ..note(id) }).unwrap();
    }
}
