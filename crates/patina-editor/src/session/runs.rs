//! The commands of the run, the node GUIs and the views (15.2, 4.3, 15.8).

use std::collections::BTreeMap;

use patina_engine::Edit;
use serde::Serialize;
use tauri::{State, WebviewWindow};

use super::{Editor, View};
use crate::engine::{GuiInput, Job};
use crate::node_process::{Ask, ToNode};
use crate::views::{ViewInput, ViewReply};

/// Builds and runs the graph, and keeps it live (15.10).
#[tauri::command(async)]
pub fn run(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you run it (11).".to_owned());
        }
        session.project.check().map_err(|e| e.to_string())?;
        editor.engine.go();
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
        editor.engine.go();
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
            editor.engine.send(Job::Bake);
        }
        Ok(session.view(&editor.engine, None, None))
    })
}

/// Sets the frames each second of the project (9.1). A live graph runs again at the new rate, as
/// after a parameter change. Save writes the rate into `patina.toml` (15.10).
#[tauri::command(async)]
pub fn frame_rate(editor: State<'_, Editor>, rate: u32) -> Result<View, String> {
    editor.with(|session| {
        session.project.set_frame_rate(rate).map_err(|e| e.to_string())?;
        session.keep_recovery(&editor.data);
        Ok(session.view(&editor.engine, Some(false), None))
    })
}

/// Stops the live runs. The active run finishes (15.2).
#[tauri::command(async)]
pub fn pause(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        session.live = false;
        editor.engine.send(Job::Pause);
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
        editor.engine.go();
        session.live = false;
        editor.engine.send(Job::Step);
        Ok(session.view(&editor.engine, None, None))
    })
}

/// Stops the live runs, and asks the active run to stop (15.2). **Run** removes the request. When
/// a node still runs at a later **Stop**, the node process ends (16.1).
#[tauri::command(async)]
pub fn stop(editor: State<'_, Editor>) -> Result<View, String> {
    editor.engine.stop();
    pause(editor)
}

/// Gives user input to the GUI of node `id` in size `mode` (0 node body, 1 **Inspector**, 2 full),
/// or asks for its widget lines when `events` is empty (4.3). A change runs a live graph again.
#[tauri::command(async)]
pub fn gui(editor: State<'_, Editor>, id: String, mode: u32, events: String) -> Result<(), String> {
    editor.with(|session| {
        editor.engine.send(Job::Gui(GuiInput { id, mode, events, rerun: session.live }));
        Ok(())
    })
}

/// Gives the nodes in the view of the graph. The frames draw only their GUIs (4.3 rule 3).
#[tauri::command(async)]
pub fn visible(editor: State<'_, Editor>, ids: Vec<String>) {
    editor.engine.send(Job::Visible(ids));
}

/// The ID of the node that runs, or that ran last in the active run. None between runs (15.2).
#[tauri::command(async)]
pub fn running(editor: State<'_, Editor>) -> Option<String> {
    editor.engine.ask(false, |id| ToNode::Ask { id, ask: Ask::Running }).unwrap_or_default()
}

/// Fires the `trigger` node `id` of the live run with `text` (9.4). The event run starts on its own
/// thread, and its result comes as a `Fired` event.
#[tauri::command(async)]
pub fn fire(editor: State<'_, Editor>, id: String, text: String) -> Result<(), String> {
    match editor.engine.ask(false, |ask| ToNode::Ask { id: ask, ask: Ask::Fire { id, text } })? {
        true => Ok(()),
        false => Err("The event domain does not run. Click **Run** to start it.".to_owned()),
    }
}

/// The live value of output `from` ("<node id>.<output key>") after the last run, with its full
/// text and its widgets (15.7, 5.3). A read never changes the run (9.2).
#[tauri::command(async)]
pub fn value(editor: State<'_, Editor>, from: String) -> Option<Box<serde_json::value::RawValue>> {
    editor.engine.ask(false, |id| ToNode::Ask { id, ask: Ask::Value(from) }).unwrap_or_default()
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
#[allow(clippy::too_many_arguments)] // The command keeps the node GUI event fields.
pub fn view_ui(window: WebviewWindow, editor: State<'_, Editor>, name: String, instance: String, events: String, selected: Vec<String>, gesture: Option<u64>) -> Result<ViewFrame, String> {
    let (root, graph) = editor.with(|session| match session.trusted {
        true => Ok((session.project.root().to_owned(), session.project.graph().clone())),
        false => Err("Trust the project before you open a view.".to_owned()),
    })?;
    let input = |ask| Job::View(Box::new(ViewInput { root, name, instance, events, graph, selected, ask }));
    let reply: ViewReply = editor.window_ask(window.label(), input)?;
    let view = match reply.edits.is_empty() {
        true => None,
        false => Some(editor.change(|project, _| {
            let edit = Edit::Group { edits: reply.edits };
            match gesture {
                Some(gesture) => project.apply_gesture(&edit, gesture),
                None => project.apply(&edit),
            }
        })?),
    };
    Ok(ViewFrame { tree: reply.tree, view, select: reply.select })
}

/// Drops the view instance of the dock slot `instance`, so the next input makes a new state (15.8).
#[tauri::command(async)]
pub fn close_view(editor: State<'_, Editor>, instance: String) -> Result<(), String> {
    editor.engine.ask(false, |ask| ToNode::Job(Job::DropView(instance, ask)))
}
