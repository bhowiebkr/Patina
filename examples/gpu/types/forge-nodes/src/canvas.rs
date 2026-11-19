//! Editor-only graph organization. These records never create shader passes.
use crate::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    /// Original import metadata; never interpreted as executable code or shader parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
    #[serde(default)]
    pub frames: Vec<Frame>,
    #[serde(default)]
    pub notes: Vec<Note>,
}
impl Canvas {
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.notes.is_empty() && self.source.is_none()
    }
    pub fn next_id(&self) -> u32 {
        self.frames
            .iter()
            .map(|f| f.id)
            .chain(self.notes.iter().map(|n| n.id))
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub id: u32,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Original sRGB group tint, when the source supplies one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_color_tag: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    pub pos: [f32; 2],
    pub size: [f32; 2],
    #[serde(default)]
    pub nodes: Vec<NodeId>,
    #[serde(default)]
    pub notes: Vec<u32>,
    #[serde(default)]
    pub folded: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: u32,
    pub text: String,
    pub pos: [f32; 2],
    pub size: [f32; 2],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterPreset {
    pub name: String,
    pub values: std::collections::BTreeMap<String, Vec<f32>>,
}

impl Canvas {
    pub fn inside(&self, frame: u32, ancestor: u32) -> bool {
        let mut current = Some(frame);
        for _ in 0..64 {
            let Some(id) = current else {
                break;
            };
            if id == ancestor {
                return true;
            }
            current = self
                .frames
                .iter()
                .find(|f| f.id == id)
                .and_then(|f| f.parent);
        }
        false
    }
    pub fn hidden_frame(&self, frame: u32) -> bool {
        let mut parent = self
            .frames
            .iter()
            .find(|f| f.id == frame)
            .and_then(|f| f.parent);
        for _ in 0..64 {
            let Some(id) = parent else {
                break;
            };
            let Some(frame) = self.frames.iter().find(|f| f.id == id) else {
                break;
            };
            if frame.folded {
                return true;
            }
            parent = frame.parent;
        }
        false
    }
    pub fn validate(&self, ids: &std::collections::BTreeSet<NodeId>) -> Result<(), String> {
        if self.frames.len() + self.notes.len() > 10000 {
            return Err("Too many graph annotations".into());
        }
        if self
            .source
            .as_ref()
            .is_some_and(|s| s.to_string().len() > 8 << 20)
        {
            return Err("Source presentation metadata exceeds 8 MiB".into());
        }
        let parents: std::collections::BTreeMap<_, _> =
            self.frames.iter().map(|f| (f.id, f.parent)).collect();
        let mut seen = std::collections::BTreeSet::new();
        let valid = |p: [f32; 2], s: [f32; 2]| {
            p.into_iter()
                .chain(s)
                .all(|v| v.is_finite() && v.abs() < 1e8)
                && s.into_iter().all(|v| v > 0.)
        };
        for n in &self.notes {
            if !seen.insert(n.id) || !valid(n.pos, n.size) || n.text.len() > 1_000_000 {
                return Err("Invalid graph note".into());
            }
        }
        let note_ids = seen.clone();
        let mut members = std::collections::BTreeSet::new();
        let mut note_members = std::collections::BTreeSet::new();
        for f in &self.frames {
            let mut parent = f.parent;
            let mut ancestors = std::collections::BTreeSet::from([f.id]);
            while let Some(id) = parent {
                if !ancestors.insert(id) || ancestors.len() > 64 {
                    return Err("Invalid graph frame hierarchy".into());
                }
                parent = *parents.get(&id).ok_or("Missing parent graph frame")?;
            }
            if !seen.insert(f.id)
                || !valid(f.pos, f.size)
                || f.title.len() > 1_000_000
                || f.description.len() > 1_000_000
                || f.nodes
                    .iter()
                    .any(|id| !ids.contains(id) || !members.insert(*id))
                || f.notes
                    .iter()
                    .any(|id| !note_ids.contains(id) || !note_members.insert(*id))
            {
                return Err("Invalid graph frame".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_dangling_duplicate_and_nonfinite_annotations() {
        let ids = [NodeId(1)].into_iter().collect();
        let mut canvas = Canvas {
            source: None,
            frames: vec![],
            notes: vec![Note {
                id: 1,
                text: "Note".into(),
                pos: [0., 0.],
                size: [200., 100.],
            }],
        };
        assert!(canvas.validate(&ids).is_ok());
        canvas.notes[0].pos[0] = f32::NAN;
        assert!(canvas.validate(&ids).is_err());
        canvas.notes[0].pos[0] = 0.;
        canvas.frames.push(Frame {
            id: 2,
            title: "Group".into(),
            description: String::new(),
            color: None,
            source_color_tag: None,
            parent: None,
            pos: [0., 0.],
            size: [300., 200.],
            nodes: vec![NodeId(2)],
            notes: vec![1],
            folded: false,
        });
        assert!(canvas.validate(&ids).is_err());
        canvas.frames[0].nodes = vec![NodeId(1)];
        assert!(canvas.validate(&ids).is_ok());
        canvas.frames[0].notes.push(1);
        assert!(canvas.validate(&ids).is_err());
        canvas.frames[0].notes.pop();
        canvas.frames[0].parent = Some(2);
        assert!(
            canvas.validate(&ids).is_err(),
            "Self-parented frames must be rejected"
        );
        canvas.frames[0].parent = Some(99);
        assert!(
            canvas.validate(&ids).is_err(),
            "Missing source groups must be rejected"
        );
    }
}
