# Automation graph layout refresh

- **Status:** done.
- **Date:** 2026-11-14.
- **Request:** Improve mockup 6. Equal node sizes made the graph difficult to follow.
- **File:** [mockup-6-automation-graph.png](../../../docs/mockups/mockup-6-automation-graph.png), replaced in place.
- **Method:** Built-in image generation; one edit call, no extra variants.
- **Changes:** Six differently sized nodes in a continuous left-to-right chain.
  Collapsed sidebar. Short connections replace the long return wire.
  Webhook is compact, Score Lead is largest, and other nodes fit their controls.
  Colored headers remain. Score Lead selection uses light pink.
- **Checks:** Inspected generated image. Six nodes, five connections and original
  sequence are visible. Saved PNG is 1585 x 992; SHA-256 matches generated original.
- **Limits:** Visual mockup only. Example payload, scores, service results and times
  are illustrative. Some node domain/kind icons are simplified or omitted.
  A tiny wire-arrow detail between CRM and Slack is imperfect.
  Larger native resolution requested but not returned; no upscaling.
- **Git:** No .git directory; no commit or push possible.

## Exact prompt

```text
Edit the attached Patina automation-graph screenshot into ONE improved replacement. Keep the same six-node lead automation, dark Windows editor styling, category-colored headers and meaningful custom controls. Redesign its GRAPH LAYOUT and NODE DIMENSIONS to make the dependency chain immediately readable. Native high resolution if available, landscape screenshot approximately16:10. No monitor/perspective, no extra variants.

CRITICAL: eliminate the six equal cards and the long backward wrapping wire. Arrange all SIX nodes in ONE clearly readable LEFT-TO-RIGHT CHAIN, using almost the entire window width. Collapse the Project sidebar to a narrow icon rail to gain room. Nodes have clearly different WIDTHS and HEIGHTS fitted to their content. The Webhook is a small trigger, Validate Lead a slim rule checker, Enrich Company a medium data card, Score Lead the largest rich editor, Upsert CRM a medium mapping node, Notify Slack a wide message-preview node. Do NOT use equal-height cards, same top edges, a dashboard grid, rows of three, or a giant return wire. No node overlaps, cropped nodes or wires through controls. Keep body text crisp and readable by simplifying secondary details, not by shrinking everything.

Suggested geometry on a1586x992 logical canvas:
Collapsed left rail40px.
Webhook x60 y345 width155 height175.
Validate Lead x250 y295 width175 height260.
Enrich Company x460 y250 width205 height335.
Score Lead x700 y190 width265 height465.
Upsert CRM x1005 y285 width215 height300.
Notify Slack x1260 y260 width275 height350.
Leave 35–40px wire gutters. All bodies have different dimensions, small trigger versus large scoring editor. Keep named edge sockets at approximately y420 so the short wires run cleanly left to right; interior controls avoid sockets. Nodes stand freely on the square grid with generous space above/below, not in section boxes. This geometry is guidance and must not appear as text.

Top bar preserve P PATINA, Lead Intake / Main Graph, LIVE, undo redo,100%,fit/grid,Run,Pause,Bake,Views, Windowscaptionbuttons. Collapsed project sidebar onlyicons. Modest bottom Logs panel with the original run#1042 success messages, slim status Ready /0errors/Lead Intake/Fused. No inspector or code editor.

Headers: Webhook rust red; Validate Lead teal; Enrich Company amber; Score Lead magenta; Upsert CRM and Notify Slack purple. Small white node-kind/domain icons, separate green ready check. Bodies charcoal, normal controlsgray. Selected Score Lead uses a LIGHT PINK outline and focused controls derived from its MAGENTA header, not cyan. Node titles stay legible. Ready can be a compact checked green dot without repeated word if space limited.

WEBHOOK smallest compact trigger:
POST pill and path /leads.
Listening green dot.
Small "Payload ▸" collapsed disclosure, don't waste space on a big full payload.
"Send test" small button.
Output Lead on right; no input.
Footer Run3ms.

VALIDATE LEAD slim:
Input Lead, output Valid.
Three compact checked rules: "Email · valid", "Company · required", "Website · URL".
"Trim whitespace" checkbox.
"3 checks passed" small status.
Footer Run1ms.
No repeated large dropdowns; the rule details can expand on demand.

ENRICH COMPANY medium-tall:
Input Lead, output Enriched.
Company API dropdown, Connected small indicator.
A custom company card: NS initials, Northstar; Software;85employees;Canada.
"Preview response" smallbutton.
Footer Run420ms.

SCORE LEAD largest, selected:
Input Enriched, output Scored.
Prominent82/100 and horizontal score bar with threshold marker70.
Three rule rows with compact numeric controls: Work email25, Company size35, Industry fit40.
"Qualified at70" field and "Qualified" result.
Preserve space for the custom scoring GUI: make this node substantially larger than Webhook and Validate Lead, not merely a different border.
Footer Run2ms.

UPSERT CRM medium:
Input Scored, output Contact.
HubSpot dropdown, "Create or update".
Small fieldmapping table: email→lead.email, company→lead.company, score→lead.score.
"Match by email" checkbox.
"Contact#4821 updated" compact status.
Footer Run310ms.

NOTIFY SLACK wide preview:
Input Contact, no outputneeded.
Channel#sales-leads, "Qualified leads only" checkbox.
A custom message preview in a dark inset, with restrained Slackicon:
"New qualified lead"
"Maya Chen · Northstar"
"Score82/100"
"CRM contact#4821"
Small Previewbutton and Message sent check.
Footer Run180ms.

WIRES: exactly five short SOLID neutral lightgray curved wires with small arrowheads. Webhook Lead -> Validate Lead Lead -> Enrich Company Lead -> Score Lead Enriched -> Upsert CRM Scored -> Notify Slack Contact. Each wire joins a visible output socket to the next node's input socket. NO long loop, no branching, no crossing, no wiring into panels. All nodes are eventdomain.

Keep the workflow's hierarchy understandable at a glance through truly different node silhouettes and clear sequential layout. Preserve named example data and colored-header family while making routine nodes compact. One replacement screenshot only.
```

