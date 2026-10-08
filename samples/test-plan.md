# Test plan: `test-plan.dwg`

A small apartment, drawn in metres, made by `cargo run -p qiyas-core --example write_test_plan -- samples/test-plan.dwg`. Every number below is exact by construction, so you can check Qiyas against it.

## What is in the drawing

| Layer | What | Where |
|---|---|---|
| A-WALL | Outer wall ring 12 × 9 m (closed polyline) | corners (0,0) to (12,9) |
| A-WALL | Partition line x = 7, 9 m long | (7,0) to (7,9) |
| A-WALL | Partition line y = 5, 5 m long | (7,5) to (12,5) |
| A-WALL | Bay window arc, radius 2 m, half circle | centre (3.5, 9), bulging up |
| A-ROOM | Living room polyline 6.7 × 8.7 m | inset 0.15 m inside the left half |
| A-ROOM | Bedroom polyline 4.7 × 4.7 m | bottom right |
| A-ROOM | Kitchen polyline 4.7 × 3.7 m | top right |
| A-ROOM | Fountain circle, radius 1.5 m | centre (3.5, 4.5) |
| A-DOOR | 3 DOOR blocks (0.9 m leaf and swing) | (3,0), (7,2), (7,7) |
| A-WINDOW | 5 WINDOW blocks (1.2 m line) | top, right and left walls |
| S-COLUMN | 6 COLUMN blocks (0.4 × 0.4 m square) | the six wall corners and junctions |
| A-TEXT | Room names and a title | |

## Expected quantities

| Measure | Click or tool | Exact value | Qiyas shows |
|---|---|---|---|
| Outer wall ring, area | Select tool, click the right wall at about (12, 4) twice | 108.000 m² | 108.000 m² |
| Outer wall ring, perimeter | Distance tool round the four corners | 42.000 m | 42.000 m |
| Partition x = 7 | Select, click the line at (7, 4) twice | 9.000 m | 9.000 m |
| Partition y = 5 | Select, click the line at (10, 5) twice | 5.000 m | 5.000 m |
| Bay arc length | Select, click the arc at (3.5, 11) twice | 6.283 m (2π) | 6.282 m |
| Fountain circle area | Select, click the circle at (5, 4.5) twice | 7.069 m² (2.25π) | 7.062 m² |
| Living room area | Select, click its left edge at (0.15, 4) twice | 58.290 m² | 58.290 m² |
| Living room perimeter | Distance tool round its corners | 30.800 m | 30.800 m |
| Bedroom area | Select, click its right edge at (11.85, 2.5) twice | 22.090 m² | 22.090 m² |
| Kitchen area | Select, click its right edge at (11.85, 7.5) twice | 17.390 m² | 17.390 m² |
| Doors | Count tool, click any door | 3 pcs | 3 pcs |
| Windows | Count tool, click any window | 5 pcs | 5 pcs |
| Columns | Count tool, click any column | 6 pcs | 6 pcs |
| Hide A-DOOR, count doors again | Layers panel, then Count tool on another door? None visible | 0 pcs | 0 pcs |

The arc and the circle come out 0.02 % and 0.1 % short because curves are measured along short straight pieces (0.001 m tolerance). Exact arc maths is on the Phase 1b list.

## Also worth trying

- Area tool: click the four corners of the bedroom polyline yourself, double-click to finish: 22.090 m².
- Rename a row, delete a row, Ctrl+Z, Ctrl+Y.
- Export take-off: the Excel file should list every row with these values and totals per kind.
- Ctrl+S: a `test-plan.dwg.bak` appears next to the file; reopen and the measurements are still there even if you delete `test-plan.qiyas`.
- Switch to Arabic: labels flip to the right, the ribbon reads قياس.

## BOQ check

`test-boq.xlsx` is a six-item BOQ for this drawing, made by
`cargo run -p qiyas-boq --example write_test_boq -- samples/test-boq.xlsx`. Two quantities are wrong on purpose.

| Code | Item | Unit | BOQ says | Drawing | Link to |
|---|---|---|---|---|---|
| A.1 | Block walls 200 mm | m | 42.000 | 42.000 | outer ring perimeter |
| A.2 | Partition walls | m | 14.000 | 14.000 | both partitions (9 + 5) |
| A.3 | Floor screed, living room | m² | 58.290 | 58.290 | living room area |
| A.4 | Floor screed, bedroom | m² | **30.000** | 22.090 | bedroom area |
| B.1 | Doors 900 mm | pcs | 3 | 3 | DOOR count |
| B.2 | Windows 1200 mm | pcs | **4** | 5 | WINDOW count |

Measure the six, import the BOQ, link each measurement to its row, and press Compare. The
conflicts panel must show exactly two quantity mismatches: **A.4** (30.000 vs 22.090 m², −26.4 %)
and **B.2** (4 vs 5 pcs, +25.0 %). Nothing else.

`cargo run -p qiyas-ai --example phase2_walkthrough` does all of this through the AI tools and
prints the conflicts. Without links (`-- --no-link`) the matching rules find nothing, because the
default descriptions ("LwPolyline on A-ROOM") do not look like the BOQ text: link, or describe the
measurements the way the BOQ does.
