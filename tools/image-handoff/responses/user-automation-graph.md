# Automation graph mockup

> Superseded by the [layout refresh](user-automation-layout-refresh.md).
> The image at the same path was replaced. The notes below describe the earlier render.

- **Status:** done.
- **Updated:** 2026-11-14 (America/Toronto).
- **Request:** One mockup of a typical n8n-style automation graph with custom node GUIs.
- **Files:** `docs/mockups/mockup-6-automation-graph.png`.
- **Generation:** Built-in image generation; one call, no extra variants.
- **Reference:** `docs/mockups/mockup-1-main-editor-colored-headers.png`.
- **Checked:** Six nodes and their custom controls are visible. The sequence is
  Webhook -> Validate Lead -> Enrich Company -> Score Lead -> Upsert CRM ->
  Notify Slack. The return wire follows the gutter and points into Score Lead.
  Event nodes use solid wires and run IDs appear in Logs. Colored headers,
  neutral bodies, and a cyan selected-node outline follow the visual direction.
  The saved PNG is 1586 x 992 pixels; verified exact copy with SHA-256.
- **Limits:** Native 3200 x 2000 was requested but not returned. No upscaling.
  Service connections, results, payload, and times are illustrative. No real
  services were connected and no automation was executed. This is a visual
  proposal, not a finalized category palette or implemented node library.
  The generator used blue checked controls rather than entirely neutral controls.
- **Git:** No Git repository; no commit or push was possible.

## Workflow inspiration

The lead-intake, enrichment, CRM, and notification sequence is informed by
[n8n's lead enrichment workflow example](https://n8n.io/workflows/11371-automate-lead-enrichment-with-web-scraping-gpt-4o-airtable-and-slack-notifications/).
Patina's node content and visual design were composed for this mockup.

## Exact generation prompt

```text
Use case: ui-mockup.
Create exactly ONE new Patina Windows editor mockup showing a typical n8n-style business automation graph, with rich custom GUIs INSIDE its nodes. Use the attached Patina screenshot as the visual reference: same logo, dark graphite palette, subtle square grid, sharp Segoe UI-like typography, colored narrow node headers, neutral node bodies, thin gray borders, small corner radii and cyan selected-node outline. This is Patina, not n8n branding. Preserve the visual family of the existing mockups.

Scenario: an inbound website lead arrives via webhook, its fields are validated, the company is enriched, custom qualification rules score the lead, a CRM contact is upserted, and the sales team receives a Slack notification. Show a completed successful example run. These are illustrative project-defined nodes, not a claim of implemented integrations.

Framing: one complete app window, landscape approximately 16:10, native 3200 x 2000 if supported, otherwise largest native size. No upscaling, no perspective, no monitor or wallpaper. Prioritize readable custom controls and an intelligible graph. Narrow left sidebar (about 12% width), top toolbar, almost all remaining area used by six LARGE nodes arranged in TWO ROWS OF THREE. This allows each node to show a genuinely detailed custom interface. No inspector or code editor; the node GUIs are the focus. Small bottom logs panel and status bar.

Top bar matches reference: Patina P logo, PATINA, path "Lead Intake / Main Graph", green LIVE dot, undo, redo, zoom "100%", fit, grid, "Run", "Pause", subdued "Bake", "Views", Windows caption buttons.
Left sidebar: tabs "Project" and "Layers", Project active. Tree under "Lead Intake": Graphs > Main Graph; Nodes > Webhook, Validate Lead, Enrich Company, Score Lead, Upsert CRM, Notify Slack; Views; Widgets; Types. Simple neutral icons, no rainbow sidebar boxes, no old edit layers.

Graph geometry: three evenly spaced columns with wide clear gutters. Top row from left to right: Webhook, Validate Lead, Enrich Company. Bottom row from left to right: Score Lead, Upsert CRM, Notify Slack. Every node is fully visible. All six are event-domain nodes with a small bell symbol in the header. Trigger has a lightning kind symbol; async network nodes use a small hourglass kind symbol; pure transforms use an italic f. Small separate green check dot indicates Ready. Named round pins on left/right edges. Neutral charcoal bodies; only the narrow title strip is colored. Rich but restrained vertical header gradients, like the reference. White titles. Native-looking gray dropdowns, fields, checkboxes and buttons. Each footer separately labels run duration.

1. TOP LEFT, title "Webhook", deep red/rust header for an event trigger. No input pins; output "Lead".
Inside: HTTP method badge "POST" next to path field "/leads". A small "Listening" status with a green dot. Compact incoming-payload preview in a dark inset:
name     Maya Chen
company  Northstar
email    maya@northstar.example
Button "Send test event". Small label "Run #1042". Footer "Run 3 ms".
This must look like a custom payload/testing panel embedded in a node, not rows of generic input pins.

2. TOP CENTER, title "Validate Lead", teal header for data processing. Input "Lead", output "Valid".
Inside: a rule-builder mini-table, labels "Field" and "Rule", three rows:
Email       Valid email
Company     Required
Website     Valid URL
Each row has a compact checked checkbox. Below, an on/off toggle labeled "Trim whitespace", then a subtle result badge "3 checks passed". Footer "Run 1 ms".

3. TOP RIGHT, title "Enrich Company", amber header for external data input. Input "Lead", output "Enriched".
Inside: "Source" dropdown "Company API". Small neutral "Connected" badge. A compact fetched-company card with initials NS, company name "Northstar", and rows:
Industry    Software
Employees   85
Country     Canada
A small "Preview response" button. Footer "Run 420 ms". No API keys, secret strings or real credentials.

4. BOTTOM LEFT, title "Score Lead", berry/magenta header for custom logic. Input "Enriched", output "Scored". SELECTED with cyan outline.
The most custom GUI of the six: a compact qualification dashboard with a large readable number "82 / 100", a neutral horizontal score bar, and a labeled threshold marker at 70. Three editable rule rows with small numeric fields:
Work email       25
Company size     35
Industry fit     40
A slider or numeric stepper "Qualified at 70". A clearly labeled result "Qualified" with a small green success icon. Tiny note "Weighted fit score". Footer "Run 2 ms".
Keep scoring colors neutral except status; don't use decorative red/yellow/green gauges.

5. BOTTOM CENTER, title "Upsert CRM", purple header for an external action/output. Input "Scored", output "Contact".
Inside: "CRM" dropdown "HubSpot". "Operation" dropdown "Create or update". Mapping mini-table:
email      lead.email
company    lead.company
score      lead.score
Small checkbox "Match by email". A compact result label "Contact #4821 updated". Footer "Run 310 ms".

6. BOTTOM RIGHT, title "Notify Slack", SAME purple header because it is also an external action/output. Input "Contact"; no required output.
Inside: "Channel" dropdown "#sales-leads". Checkbox "Qualified leads only". A custom MESSAGE PREVIEW card, not code:
New qualified lead
Maya Chen · Northstar
Score 82 / 100
CRM contact #4821
A small "Preview" button and separate status text "Message sent". Footer "Run 180 ms".

WIRES must show the exact sequential dependency:
Webhook Lead -> Validate Lead Lead -> Enrich Company Lead.
Then Enrich Company Enriched -> Score Lead Enriched.
Then Score Lead Scored -> Upsert CRM Scored -> Notify Slack Contact.
The long Enrich-to-Score wire is intentionally routed in the EMPTY gutter BETWEEN THE TWO ROWS: leave the right edge of Enrich, curve around its bottom-right outside the node, run leftward through the inter-row gutter, then curve down at the far left of the bottom row and into Score Lead's left input. Use subtle small arrowheads along this long return wire so its direction is clear. Keep this routing well outside all controls and text. No wire crosses a node body. All wires are solid neutral light gray; all nodes are in the event domain so NO dashed wires. Other wires are short, tidy smooth curves.

Bottom panel, compact: tabs "Logs", "Output", "Debug", Logs active. Show three short monospaced lines:
"[#1042] Webhook received one lead"
"[#1042] CRM contact #4821 updated"
"[#1042] Slack message sent · Run completed in 916 ms"
Status bar: green checked "Ready", "0 errors", "Project: Lead Intake", "Fused".

Keep the workflow coherent, all text clean, controls readable, and category colors confined to node headers. No large diagram captions, no Rust logos, no obsolete layer list, no history panel or retry settings, no implementation jargon, no promotional badges, no extra seventh node. The composition should clearly communicate what an automation tool built with Patina's custom nodes looks like.
```

