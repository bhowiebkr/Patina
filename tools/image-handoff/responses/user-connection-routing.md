# Connection routing and appearance

**Status:** done
**Updated:** 2026-11-14 (America/Toronto)

The owner asked to make connections a first-class part of the node interface.
Cover routing options, manually placed bends, routes around nodes, ribbon
crossings, and room for other connection kinds. Automatic routing and raised
bridges are optional design directions, not requirements for the first build.
The owner supplied a game screenshot and requested one Patina interpretation.

## Delivered

- `docs/mockups/mockup-16-routed-connections.png`, native 1586 x 992, not upscaled.
- [GUI guide](../../../docs/gui-visual-guide.md#node-connections): a first-class connection section, manual routing, crossings, optional locks and other connection cases.
- [Editor spec 15.9](../../../docs/spec/15-editor.md#159-connections-and-routing): the shared interface rules; automatic routing and elaborate bridges remain optional.
- [Circuit design notes](../../../docs/circuit-graph-design.md#possible-routing-algorithms): primary references for obstacle avoidance, routing penalties and checkpoints. No routing library was selected or installed.

The image shows two independent ribbons going around an unrelated node. The
violet cable passes over green at a raised bridge. Square handles and a lock
badge demonstrate route editing. The game image was used as visual inspiration;
its actual controls were not tested.

**Checked:** inspected the generated image, confirmed dimensions and identical
saved bytes, reviewed focused documentation diffs and checked local links.
No app implementation, routing algorithm or GUI behavior was tested.

**Limits:** some painted bit strips have extra cells and contact lights disagree
with the shown values. The guide supplies the exact intended patterns. Bridges,
automatic avoidance and locks are design options, not existing capabilities.

**Git:** this directory has no Git repository. No commit or push was created.

## Generation

Built-in image generation. One render. The two recent reference images are
the owner's game screenshot (routing and crossover inspiration) and
`docs/mockups/mockup-12-ribbon-connections.png` (Patina appearance).
Requested 3200 x 2000; actual output dimensions will be recorded after saving.

```text
Create ONE polished Patina GUI mockup showing OPTIONAL ROUTING AROUND NODES AND A RIBBON CROSSOVER BRIDGE. Landscape 3200x2000 if supported. Use the two most recent reference images: the user's game screenshot is inspiration ONLY for the way routed wires use space and jump over each other with dark mechanical collars; the most recent Patina "Ribbon connections" screenshot is the ACTUAL STYLE AUTHORITY for neutral charcoal, Patina blue faceted P/white wordmark, compact colored headers, clean controls and beautifully rounded individual ribbon conductors. Do not copy the game's blue checkerboard, code panels or busy layout. This must look like a natural next view of Patina.

Connections are the focus. Five nodes maximum: two sources on left, two corresponding readers on right, one smaller unrelated node in the center that both routes visibly avoid. Lots of clear canvas space. No inspector sidebar, no bottom log panel. Thin top toolbar with PATINA, "Routed connections", "Route: Orthogonal", "Corners: Rounded", "Crossings: Bridge". Neutral dark grid #181818; node bodies #262626, fields #191919, borders #454545, text #EEEEEB. Restrained amber focus. Each node differs in size to fit its controls, header colors confined to the header. Straight-on professional screenshot with subtle 2.5D electrical cable finish, no neon bloom, photoreal electronics, perspective or unrelated decorations.

LAYOUT AND LOGICAL CONNECTIVITY, approximate coordinates at1600x1000:
Source1 at(75,160) size(290,210), amber header "Address", value0x3C /60, exact8bit strip00111100, right edge socket addr[7:0].
Source2 at(75,660) size(300,230), amber header "Data", value0x66 /102, exact8bit strip01100110, right edge socket data[7:0].
Receiver1 at(1240,150) size(285,210), purple header "Data Monitor", left edge data[7:0], value0x66, samebits.
Receiver2 at(1240,610) size(280,230), copper header "Address Register", left edge addr[7:0], value0x3C, samebits.
Central unrelated small teal-header "Waveform" node around(710,360) size(220,180), compact waveform plot, Frequency1.0 Hz field and an unconnected sample output. It is there to demonstrate routing clearance, not part of the two buses. Do not invent wires through this node or its controls. Keep at least30px of clear space between all cables and this node.

ROUTES:
GREEN address eight-lane ribbon exits left-top Address horizontally to x590, then turns DOWN with a broad smooth concentric90degree bend, travels vertically outside the LEFT edge of central Waveform, then turns RIGHT below it at y640 and continues into Address Register input. Its whole cable stays outside central node bounds. Label a horizontal stretch "addr[7:0] / 8".
VIOLET data eight-lane ribbon exits left-bottom Data horizontally along y770 to x1080, then turns UP at x1080, travels vertically outside the RIGHT edge of central Waveform, then turns RIGHT at y270 into Data Monitor input. Its whole cable stays outside central node bounds. Label a stretch "data[7:0] / 8".
These specific two independent routes CROSS ONCE at approximately(x1080,y640). That crossing is the HERO. VIOLET vertical bus passes OVER the GREEN horizontal bus. Build a beautiful short raised graphite crossover saddle there: two subtle dark retaining collars, beveled shoulders, a slight under-shadow and an exposed violet ribbon running over the top. All eight violet conductors remain separate, retain their order, and rise gently across the saddle. The full green eight-lane ribbon passes BELOW; it is briefly occluded by the bridge and visibly continues on the same alignment and with the same width on the other side. Neither bus changes identity. NO junction dot, NO mixing lanes, NO fan-out. A very small neutral pill next to the crossing reads "Crossing - not joined". The bridge is a compact cable accessory, not a processing node. It should be large enough to understand at this scale but smaller than the Waveform node.

Cable appearance: distinguishable insulated strands with fine black gutters, narrow highlights, restrained soft contact shadows, rounded outer edges. Each ribbon contains8lanes, about3-4px perlane at1600pixels, with dim and bright shades in its own green or violet family. Brightness matches value; low lanes remain visible. Connector housings seat all lane ends at each node edge. Broad concentric rounded corners, no hard elbows, tangled splines, contact with unrelated node bodies or missing endpoints. Ribbons must follow the routes described, not direct shortest splines.

MANUAL ROUTING DETAILS: two small square route handles on the selected violet route, one near the long lower horizontal segment and one beside the upper turn. One small amber padlock badge marks a fixed bend. A compact contextual floating strip near top center says "Selected: data[7:0]" with checkbox "Lock bends" checked, "Avoid nodes" checked and a small "Reset route" action. This strip is UI, no pins. Locking bends means preserving a user's route shape, not locking electrical values. Avoid a giant tutorial legend.
Bottom subtle status "Routing concept / 2 independent buses". The result should make the usefulness of intentional wrapping, manual bends and a raised wire crossing obvious while retaining Patina's existing visual language. Make one image only.
```
