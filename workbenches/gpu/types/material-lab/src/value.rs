use crate::World;
use forge_nodes::{NativeImage, Socket};
use patina_sdk::{Param, Text, Viewer};
use std::{
    fmt::{self, Write},
    sync::Arc,
};

#[derive(Clone)]
pub struct Image {
    pub raw: NativeImage,
    pub target: Arc<World>,
}
impl std::ops::Deref for Image {
    type Target = NativeImage;
    fn deref(&self) -> &NativeImage {
        &self.raw
    }
}
impl Image {
    pub fn output(&self, layer: u32, socket: Socket) -> Self {
        Self {
            raw: self.raw.output(layer, socket),
            target: Arc::clone(&self.target),
        }
    }
}

#[derive(Clone, Default)]
pub struct BakeTarget(pub Option<Arc<World>>);
impl Param for BakeTarget {
    fn default_value() -> Self {
        Self::default()
    }
    fn from_text(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            Ok(Self::default())
        } else {
            Err("Connect a Bake target, or leave empty for the reference object".into())
        }
    }
    fn to_text(&self) -> String {
        String::new()
    }
}
impl Viewer for BakeTarget {
    fn text(&self, out: &mut Text) -> fmt::Result {
        match &self.0 {
            Some(w) => write!(
                out,
                "{} / {} maps / {} material",
                w.0.info.name, w.0.maps.size, w.1
            ),
            None => write!(out, "Reference object"),
        }
    }
}
