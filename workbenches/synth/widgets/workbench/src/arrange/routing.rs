use patina_sdk::Graph;

pub fn add(graph: &mut Graph, score: &str, track: u32) -> Result<(), String> {
    let score_wire = format!("{score}.out");
    let clock = graph
        .nodes()
        .find(|id| {
            graph.folder(id) == Some("nodes/song-clock")
                && graph
                    .wires()
                    .any(|(from, to)| from == score_wire && to == format!("{id}.score"))
        })
        .map(str::to_owned)
        .ok_or("Connect the arrangement to a song-clock node first")?;
    let output = graph
        .nodes()
        .find(|id| graph.folder(id) == Some("nodes/stereo-output"))
        .map(str::to_owned)
        .ok_or("Add a stereo-output node first")?;
    let destination = format!("{output}.sound");
    let previous = graph
        .wires()
        .find(|(_, to)| *to == destination)
        .map(|(from, _)| from.to_owned());
    let node = graph.add_node(
        "nodes/song-track",
        [850.0, 160.0 + f64::from(track) * 150.0],
    );
    graph.set_param(&node, "track", track);
    graph.connect(&score_wire, &format!("{node}.score"));
    graph.connect(&format!("{clock}.out"), &format!("{node}.time"));
    if let Some(previous) = previous {
        let mix = graph.add_node(
            "nodes/stereo-mix",
            [1470.0, 450.0 + f64::from(track) * 150.0],
        );
        graph.disconnect(&previous, &destination);
        graph.connect(&previous, &format!("{mix}.a"));
        graph.connect(&format!("{node}.out"), &format!("{mix}.b"));
        graph.connect(&format!("{mix}.out"), &destination);
    } else {
        graph.connect(&format!("{node}.out"), &destination);
    }
    Ok(())
}

pub fn remove(graph: &mut Graph, score: &str, track: u32) -> Result<(), String> {
    let nodes: Vec<_> = graph
        .nodes()
        .filter(|id| {
            graph.folder(id) == Some("nodes/song-track")
                && graph.param(id, "track").and_then(|s| s.parse::<u32>().ok()) == Some(track)
                && graph
                    .wires()
                    .any(|(from, to)| from == format!("{score}.out") && to == format!("{id}.score"))
        })
        .map(str::to_owned)
        .collect();
    if nodes.is_empty() {
        return Ok(());
    }
    if nodes.len() != 1 {
        return Err("Remove duplicate track-player routes on the canvas first".into());
    }
    let output = format!("{}.out", nodes[0]);
    let targets: Vec<_> = graph
        .wires()
        .filter(|(from, _)| *from == output)
        .map(|(_, to)| to.to_owned())
        .collect();
    if targets.len() > 1 {
        return Err(
            "Disconnect extra track routes on the canvas before removing this track".into(),
        );
    }
    let bypass = if let Some(target) = targets.first() {
        let (mix, pin) = target.rsplit_once('.').ok_or("Invalid track route")?;
        if graph.folder(mix) != Some("nodes/stereo-mix") || !matches!(pin, "a" | "b") {
            return Err("Connect this track through a stereo-mix node before removing it".into());
        }
        let other = format!("{mix}.{}", if pin == "a" { "b" } else { "a" });
        let source = graph
            .wires()
            .find(|(_, to)| *to == other)
            .map(|(from, _)| from.to_owned())
            .ok_or("The mixer needs its other source")?;
        if source == output {
            return Err("The mixer uses this track twice. Correct its route first".into());
        }
        let destinations: Vec<_> = graph
            .wires()
            .filter(|(from, _)| *from == format!("{mix}.out"))
            .map(|(_, to)| to.to_owned())
            .collect();
        Some((mix.to_owned(), source, destinations))
    } else {
        None
    };
    graph.remove_node(&nodes[0]);
    if let Some((mix, source, destinations)) = bypass {
        graph.remove_node(&mix);
        for destination in destinations {
            graph.connect(&source, &destination);
        }
    }
    Ok(())
}
