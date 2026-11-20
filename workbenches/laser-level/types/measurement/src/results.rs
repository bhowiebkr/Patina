use crate::{
    ResultData,
    core::{
        results::Results,
        survey::{DriftCorrection, Session},
    },
    survey::Snapshot,
};
use std::sync::Arc;

#[derive(Default)]
pub struct Processor {
    session: Option<Arc<Session>>,
    run: usize,
    mode: bool,
    pub output: ResultData,
}
impl Processor {
    pub fn update(&mut self, survey: &Snapshot, interpolate: bool) -> ResultData {
        if self.session.as_ref().is_some_and(|s| Arc::ptr_eq(s, &survey.session))
            && self.run == survey.run
            && self.mode == interpolate
        {
            return self.output.clone();
        }
        self.session = Some(survey.session.clone());
        self.run = survey.run;
        self.mode = interpolate;
        let correction = if interpolate { DriftCorrection::Interpolate } else { DriftCorrection::LastZero };
        self.output = match survey.run() {
            Some(run) => {
                match Results::compute(run, &run.scale(&survey.session.scale, &survey.session.sensor), correction) {
                    Ok(result) => ResultData {
                        result: Some(Arc::new(result)),
                        name: run.name.clone(),
                        error: String::new(),
                        correction,
                    },
                    Err(e) => {
                        ResultData { error: e.to_string(), name: run.name.clone(), correction, ..Default::default() }
                    }
                }
            }
            None => ResultData { error: "No survey run".into(), correction, ..Default::default() },
        };
        self.output.clone()
    }
}
