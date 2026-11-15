# Patina Specification

Status: Handoff version 1

The specification is split into these files. Each file keeps its section numbers, so a reference such as (7.2) means section 7.2 in the file that holds section 7.

| Sections | File |
|---|---|
| 0. Notes for the agent | This file |
| 1. Overview, 2. Principles, 3. Architecture | [01-overview.md](01-overview.md) |
| 4. Nodes | [04-nodes.md](04-nodes.md) |
| 5. Data types, 6. Graphs and projects | [05-types-and-projects.md](05-types-and-projects.md) |
| 7. Layers, 8. Compilation | [07-layers-and-compilation.md](07-layers-and-compilation.md) |
| 9. Execution, 10. GPU | [09-execution-and-gpu.md](09-execution-and-gpu.md) |
| 11. Security, 12. Testing, 13. Distribution, 14. Packaging | [11-security-and-release.md](11-security-and-release.md) |
| 15. Editor | [15-editor.md](15-editor.md) |
| 16. Optional targets, 17. Out of scope for version 1, 18. Open questions, 19. Milestones | [16-roadmap.md](16-roadmap.md) |

## 0. Notes for the agent

- These files are the full specification for Patina. Read all of them before you write code.
- Read the [GUI visual guide](../gui-visual-guide.md) for the palette, node sizing,
  custom controls and corrections for all current mockups. Mockup 9 is the main
  theme reference. These earlier images remain layout and error references:
  - **Main editor** ([../mockups/mockup-1-main-editor-colored-headers.png](../mockups/mockup-1-main-editor-colored-headers.png)): graph, inspector with code and parameters, logs.
  - **Error view** ([../mockups/mockup-3-error-view.png](../mockups/mockup-3-error-view.png)): compile error and a subgraph.
- The mockups show the visual direction: dark theme, layout, and density. If a mockup and this spec disagree, follow this spec. Known differences:
  - The mockups show edit layers: a layer list with on/off toggles, an orphan edit warning, "Layer: Color Grade" in the status bar, and "Layer Edit" in the inspector header. Patina has no edit layers. Its layers are compile layers: the base and the slot (7). Show them as 15.2 and 15.4 tell.
  - The inspector header uses a Rust logo. Do not use it. Use Patina node-kind icons (15.3).
  - Labels such as "Compiled 12.8 ms" mix build status and run time. Use separate labels (15.3).
  - In the error view, the node with the error has a green dot. It must have a red border and an error icon. The inspector header must show the selected node with its path. The error line must have a mark in the code. The subgraph border must be neutral gray, and the subgraph needs a collapse button.
  - The error view logs "Graph execution aborted due to compile errors". The graph does not stop. It keeps running with the last build of the node that compiled (7.2), so the other nodes keep their normal look.
  - The error view has a **Compare** tab in the bottom panel. Patina has no compare mode. The bottom panel has the Logs, Output, and Debug tabs (15.2).
- Start with milestone 0 (section 19, in [16-roadmap.md](16-roadmap.md#19-milestones)). Report the result of each prototype before you continue, because each result can change the design.
- Section 18 ([16-roadmap.md](16-roadmap.md#18-open-questions)) lists open questions. Do not decide them alone. Ask the owner.
