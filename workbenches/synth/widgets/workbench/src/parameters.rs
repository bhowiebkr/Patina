//! Bindings shared by Ember's performance surface and dock tools.
use panel::{AMBER, PINK, TEAL, VIOLET};
use patina_sdk::Graph;

pub const SEQUENCE: &str = "e6be000000000010";
pub const TIMBRE: &str = "e6be000000000011";
pub const CONTOUR: &str = "e6be000000000012";
pub const COLOR: &str = "e6be000000000013";
pub const SPACE: &str = "e6be000000000014";
pub const OUTPUT: &str = "e6be000000000026";
pub const CONSOLE: &str = "e6be000000000030";

/// One binding to the existing patch. The view never owns the saved value.
#[derive(Clone, Copy)]
pub struct Parameter {
    pub node: &'static str,
    pub folder: &'static str,
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub range: [f32; 2],
    pub default: f32,
    pub logarithmic: bool,
    pub color: [u8; 3],
}

impl Parameter {
    pub fn read(self, graph: &Graph) -> f32 {
        graph
            .param(self.node, self.key)
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| if v.is_finite() { v } else { self.default })
            .unwrap_or(0.0)
            .clamp(self.range[0], self.range[1])
    }

    pub fn editable(self, graph: &Graph) -> bool {
        available(graph, self.node, self.folder, self.key)
    }

    pub fn write(self, graph: &mut Graph, value: f32) {
        if value.is_finite() && self.editable(graph) {
            let value = value.clamp(self.range[0], self.range[1]);
            if value != self.read(graph) {
                graph.set_param(self.node, self.key, value);
            }
        }
    }
}

pub fn available(graph: &Graph, node: &str, folder: &str, key: &str) -> bool {
    graph.folder(node) == Some(folder)
        && !graph
            .wires()
            .any(|(_, to)| to.split_once('.') == Some((node, key)))
}

const fn parameter(
    node: &'static str,
    folder: &'static str,
    key: &'static str,
    label: &'static str,
    range: [f32; 2],
    default: f32,
) -> Parameter {
    Parameter {
        node,
        folder,
        key,
        label,
        unit: "",
        range,
        default,
        logarithmic: false,
        color: AMBER,
    }
}

pub const MASTER: Parameter = Parameter {
    color: TEAL,
    ..parameter(
        CONSOLE,
        "nodes/console",
        "level",
        "MASTER",
        [0.0, 1.0],
        0.65,
    )
};
pub const DRUMS: Parameter =
    parameter(CONSOLE, "nodes/console", "drums", "DRUMS", [0.0, 1.0], 0.65);
pub const BPM: Parameter = Parameter {
    unit: "BPM",
    ..parameter(
        SEQUENCE,
        "nodes/sequence",
        "bpm",
        "TEMPO",
        [40.0, 220.0],
        112.0,
    )
};
pub const SWING: Parameter = parameter(
    SEQUENCE,
    "nodes/sequence",
    "swing",
    "SWING",
    [0.0, 0.45],
    0.12,
);
pub const CUTOFF: Parameter = Parameter {
    unit: "Hz",
    logarithmic: true,
    color: TEAL,
    ..parameter(
        COLOR,
        "nodes/color",
        "cutoff",
        "CUTOFF",
        [60.0, 16000.0],
        1600.0,
    )
};
pub const RESONANCE: Parameter = Parameter {
    color: TEAL,
    ..parameter(
        COLOR,
        "nodes/color",
        "resonance",
        "RESONANCE",
        [0.0, 0.95],
        0.4,
    )
};

pub const PERFORMANCE: [Parameter; 6] = [MASTER, DRUMS, BPM, SWING, CUTOFF, RESONANCE];
pub const TONE: [Parameter; 4] = [
    Parameter {
        unit: "ct",
        ..parameter(TIMBRE, "nodes/timbre", "detune", "DETUNE", [0.0, 30.0], 9.0)
    },
    parameter(TIMBRE, "nodes/timbre", "sub", "SUB", [0.0, 1.0], 0.28),
    Parameter {
        color: TEAL,
        unit: "oct",
        ..parameter(COLOR, "nodes/color", "motion", "MOTION", [0.0, 4.0], 1.2)
    },
    Parameter {
        color: TEAL,
        unit: "Hz",
        logarithmic: true,
        ..parameter(COLOR, "nodes/color", "rate", "LFO RATE", [0.02, 8.0], 0.18)
    },
];
pub const ENVELOPE: [Parameter; 4] = [
    Parameter {
        color: PINK,
        unit: "s",
        logarithmic: true,
        ..parameter(
            CONTOUR,
            "nodes/contour",
            "attack",
            "ATTACK",
            [0.001, 2.0],
            0.008,
        )
    },
    Parameter {
        color: PINK,
        unit: "s",
        logarithmic: true,
        ..parameter(
            CONTOUR,
            "nodes/contour",
            "decay",
            "DECAY",
            [0.005, 2.0],
            0.20,
        )
    },
    Parameter {
        color: PINK,
        ..parameter(
            CONTOUR,
            "nodes/contour",
            "sustain",
            "SUSTAIN",
            [0.0, 1.0],
            0.35,
        )
    },
    Parameter {
        color: PINK,
        unit: "s",
        logarithmic: true,
        ..parameter(
            CONTOUR,
            "nodes/contour",
            "release",
            "RELEASE",
            [0.005, 3.0],
            0.24,
        )
    },
];
pub const ECHO: [Parameter; 3] = [
    Parameter {
        color: VIOLET,
        unit: "s",
        logarithmic: true,
        ..parameter(SPACE, "nodes/space", "seconds", "TIME", [0.02, 1.5], 0.402)
    },
    Parameter {
        color: VIOLET,
        ..parameter(
            SPACE,
            "nodes/space",
            "feedback",
            "FEEDBACK",
            [0.0, 0.85],
            0.42,
        )
    },
    Parameter {
        color: VIOLET,
        ..parameter(SPACE, "nodes/space", "mix", "WET", [0.0, 0.8], 0.3)
    },
];
