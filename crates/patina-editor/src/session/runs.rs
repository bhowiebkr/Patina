//! The commands of the run, the node GUIs and the views (15.2, 4.3, 15.8).

use std::collections::BTreeMap;

use patina_engine::Edit;
use serde::Serialize;
use tauri::{State, WebviewWindow};

use super::{Editor, View};
use crate::engine::{self, GuiInput, Job};
use crate::views::ViewInput;

/// Builds and runs the graph, and keeps it live (15.10).
#[tauri::command(async)]
pub fn run(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you run it (11).".to_owned());
        }
        session.project.check().map_err(|e| e.to_string())?;
        editor.shared.control.set_stop(false);
        session.live = true;
        Ok(session.view(&editor.engine, Some(true), None))
    })
}

/// Runs the open subgraph alone, with `inputs` for its public inputs, and keeps it live. No
/// `inputs` ends the test, and the main graph runs again (15.9).
#[tauri::command(async)]
pub fn test(editor: State<'_, Editor>, inputs: Option<BTreeMap<String, String>>) -> Result<View, String> {
    editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you run it (11).".to_owned());
        }
        // A new use needs a build. A change of an input value changes only the parameters.
        let build = session.test.is_some() != inputs.is_some();
        session.test = inputs;
        editor.shared.control.set_stop(false);
        session.live = true;
        Ok(session.view(&editor.engine, Some(build), None))
    })
}

/// Fuses the node in the slot back into the base now, in bake mode (7.2). Only a live graph runs
/// a build, so a paused graph fuses at the next **Run** and the time after it.
#[tauri::command(async)]
pub fn bake(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        if session.live {
            let _ = editor.engine.send(Job::Bake);
        }
        Ok(session.view(&editor.engine, None, None))
    })
}

/// Stops the live runs. The active run finishes (15.2).
#[tauri::command(async)]
pub fn pause(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        session.live = false;
        let _ = editor.engine.send(Job::Pause);
        Ok(session.view(&editor.engine, None, None))
    })
}

/// Runs one frame of the last run, and stops the live runs (9.1, 15.2).
#[tauri::command(async)]
pub fn step(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you run it (11).".to_owned());
        }
        editor.shared.control.set_stop(false);
        session.live = false;
        let _ = editor.engine.send(Job::Step);
        Ok(session.view(&editor.engine, None, None))
    })
}

/// Stops the live runs, and asks the active run to stop (15.2). **Run** removes the request.
#[tauri::command(async)]
pub fn stop(editor: State<'_, Editor>) -> Result<View, String> {
    editor.shared.control.set_stop(true);
    pause(editor)
}

/// Gives user input to the GUI of node `id` in size `mode` (0 node body, 1 **Inspector**, 2 full),
/// or asks for its widget lines when `events` is empty (4.3). A change runs a live graph again.
#[tauri::command(async)]
pub fn gui(editor: State<'_, Editor>, id: String, mode: u32, events: String) -> Result<(), String> {
    editor.with(|session| {
        let _ = editor.engine.send(Job::Gui(GuiInput { id, mode, events, rerun: session.live }));
        Ok(())
    })
}

/// The ID of the node that runs, or that ran last in the active run. None between runs (15.2).
#[tauri::command(async)]
pub fn running(editor: State<'_, Editor>) -> Option<String> {
    editor.shared.running()
}

/// Fires the `trigger` node `id` of the live run with `text` (9.4). The event run starts on its own
/// thread, and its result comes as a `Fired` event.
#[tauri::command(async)]
pub fn fire(editor: State<'_, Editor>, id: String, text: String) -> Result<(), String> {
    let remote = engine::lock(&editor.shared.remote).clone();
    match remote.is_some_and(|remote| remote.fire(&id, text)) {
        true => Ok(()),
        false => Err("The event domain does not run. Click **Run** to start it.".to_owned()),
    }
}

/// The full text of the live value of output `from` ("<node id>.<output key>") after the last run
/// (15.7). A read never changes the run (9.2).
#[tauri::command(async)]
pub fn value(editor: State<'_, Editor>, from: String) -> Option<String> {
    engine::lock(&editor.shared.values).iter().find(|v| v.from == from).map(|v| v.text.clone())
}

/// The widget lines of a view, and the new view of the project when the view wrote to the graph.
#[derive(Serialize)]
pub struct ViewFrame {
    tree: String,
    view: Option<View>,
    /// The nodes to select in the graph view, when the view selected nodes.
    select: Option<Vec<String>>,
}

/// Gives user input to view `name`, or asks for its widget lines when `events` is empty (15.8).
/// The first input builds the view. Its writes go through the edit path, so undo works.
#[tauri::command(async)]
pub fn view_ui(window: WebviewWindow, editor: State<'_, Editor>, name: String, instance: String, events: String, selected: Vec<String>) -> Result<ViewFrame, String> {
    let (root, graph) = editor.with(|session| match session.trusted {
        true => {
            let reports: Vec<_> = engine::lock(&editor.shared.reports).iter().chain(engine::lock(&editor.shared.event_reports).iter()).cloned().collect();
            Ok((session.project.root().to_owned(), patina_engine::view::snapshot(session.project.graph(), &engine::lock(&editor.shared.values), &reports, &selected)))
        }
        false => Err("Trust the project before you open a view.".to_owned()),
    })?;
    let frame = editor.window_ask(window.label(), |reply| Job::View(Box::new(ViewInput { root, name, instance, events, graph, reply })))?;
    let view = match frame.edits.is_empty() {
        true => None,
        // The writes of one call are one step of undo.
        false => Some(editor.change(|project| project.apply(&Edit::Group { edits: frame.edits }))?),
    };
    Ok(ViewFrame { tree: frame.tree, view, select: frame.select })
}

/// Drops the view instance of the dock slot `instance`, so the next input makes a new state (15.8).
#[tauri::command(async)]
pub fn close_view(editor: State<'_, Editor>, instance: String) -> Result<(), String> {
    editor.ask(|reply| Job::DropView(instance, reply))
}
