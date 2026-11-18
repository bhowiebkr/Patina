//! The sources of the nodes that the tests write into a project.

/// A connection kind of the test, for the f64 wires of the hello example (6.3).
pub const KIND: &str = "#[patina_sdk::connection]
pub fn whole(value: &f64) -> patina_sdk::Lanes {
    patina_sdk::Lanes::bits(8, *value as u64).label(\"whole\")
}
";

/// A generic stateful frame node of the test (4.2, 4.5).
pub const TICK: &str = "#[patina_sdk::node(domain = frame)]
pub struct Tick<T> {
    count: f64,
    last: Option<T>,
}

impl<T: Copy + Into<f64>> Tick<T> {
    pub fn init() -> Self {
        Tick { count: 0.0, last: None }
    }

    pub fn process(&mut self, base: T) -> f64 {
        self.last = Some(base);
        self.count = (self.count + base.into()) % 10.0;
        self.count
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        ui.label(\"Tick\");
        if ui.button(\"Reset\").clicked() {
            self.count = 0.0;
        }
        ui.slider(\"Count\", &mut self.count, 0.0..=10.0);
    }
}
";

/// A generic node of the test (4.5).
pub const MIX: &str = "#[patina_sdk::node]
pub fn mix<T: std::ops::Add<Output = T> + Copy>(a: &T, b: T) -> T {
    *a + b
}
";

/// A generic node with no input: the readers of its output give its type (4.5).
pub const ZERO: &str = "#[patina_sdk::node]
pub fn zero<T: Default>() -> T {
    T::default()
}
";

/// An audio node of the test (9.3): a block with the peak two times the level.
pub const TONE: &str = "#[patina_sdk::node(domain = audio)]
pub fn tone(level: &f64) -> patina_sdk::AudioBlock {
    patina_sdk::AudioBlock::from_fn(|i| if i == 0 { *level as f32 * 2.0 } else { 0.0 })
}
";

/// A frame node of the test that reads an audio block (9.3). Its frame and its GUI are slow, so
/// that a test sees the blocks run during a frame and during a `ui` call (4.3).
pub const METER: &str = "#[patina_sdk::node(domain = frame)]
pub struct Meter {
    peak: f64,
}

impl Meter {
    pub fn init() -> Self {
        Meter { peak: 0.0 }
    }

    pub fn process(&mut self, sound: &patina_sdk::AudioBlock) -> f64 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        self.peak = f64::from(sound.peak());
        self.peak
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        ui.label(\"Meter\");
    }
}
";

/// A stateful audio node of the test that keeps a value that is not `Sync` (9.3).
pub const HOLD: &str = "#[patina_sdk::node(domain = audio)]
pub struct Hold {
    last: std::rc::Rc<f32>,
}

impl Hold {
    pub fn init() -> Self {
        Hold { last: std::rc::Rc::new(0.0) }
    }

    pub fn process(&mut self, sound: &patina_sdk::AudioBlock) {
        self.last = std::rc::Rc::new(sound.peak());
    }
}
";

/// An audio node of the test that plays its input (9.3).
pub const SPEAKER: &str = "#[patina_sdk::node(domain = audio)]
pub fn speaker(sound: &patina_sdk::AudioBlock) {
    patina_sdk::audio::play(sound);
}
";
