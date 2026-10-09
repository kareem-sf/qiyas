# Open CAD Studio: full feature list

Source: github.com/HakanSeven12/OpenCADStudio, cloned 8 Oct 2026 (530,000 lines of Rust, GPL-3).
Taken from the code, not the README. Tick the boxes you want in the QS Mind app.

A tag after a line says why a QS might care:
- **QS** = useful for take-off and BOQ work.
- **CAD** = general drafting; only needed if we want a real CAD, not a QS tool.

## 1. Files

- [ ] 1. Open DWG (R14 to 2018) **QS**
- [ ] 2. Open DXF (R14 to 2018) **QS**
- [ ] 3. Save DWG and DXF, pick the version **CAD**
- [ ] 4. Open backup and autosave files (BAK, SV$)
- [ ] 5. Recover damaged drawings
- [ ] 6. Import PDF as vector lines and text, by layer **QS**
- [ ] 7. Attach PDF as an underlay (trace over it) **QS**
- [ ] 8. Attach a raster image (scan, photo) as an underlay **QS**
- [ ] 9. Attach point clouds **CAD**
- [ ] 10. Import OBJ meshes, export STL and STEP **CAD**
- [ ] 11. Import LandXML survey points **QS**
- [ ] 12. Export entity data to CSV **QS**
- [ ] 13. Plot to PDF, print to a printer, plot styles (CTB, STB)
- [ ] 14. External references (xref): attach, clip, manage, bind **CAD**
- [ ] 15. File tabs, recent files, file associations, thumbnails

## 2. Drawing (create geometry)

- [ ] 16. Line, polyline, 3D polyline, ray, construction line
- [ ] 17. Circle, arc, ellipse, donut, spline, helix
- [ ] 18. Rectangle and polygon shapes
- [ ] 19. Hatch and hatch edit **QS** (areas come from closed hatches)
- [ ] 20. Point, revision cloud, wipeout, centre line, trace, 2D solid
- [ ] 21. Multiline, sketch (freehand)
- [ ] 22. Raster image placement

## 3. Modify (edit geometry)

- [ ] 23. Move, copy, rotate, scale, mirror, stretch, align
- [ ] 24. Trim, extend, break, join, lengthen, reverse
- [ ] 25. Offset, fillet, chamfer, blend
- [ ] 26. Array (rectangular and polar)
- [ ] 27. Explode, flatten, overkill (remove duplicates)
- [ ] 28. Polyline edit, spline edit, multiline edit
- [ ] 29. Clipboard copy and paste between drawings
- [ ] 30. Groups

## 4. Measure (inquiry)

- [ ] 31. Distance between points **QS**
- [ ] 32. Area and perimeter of a closed shape **QS**
- [ ] 33. Measure geometry (radius, angle, volume) **QS**
- [ ] 34. Divide and measure along a path **QS**
- [ ] 35. ID a point's coordinates
- [ ] 36. Count objects, count areas, count table and count fields **QS** (counts doors, columns, fixtures)
- [ ] 37. Data extraction: pull attributes and properties into a table **QS**
- [ ] 38. Data link to a spreadsheet **QS**

## 5. Annotation

- [ ] 39. Single text, multi-line text, text edit, find and replace
- [ ] 40. Dimensions: linear, aligned, angular, radius, diameter, arc length, ordinate, jogged, baseline, continue, quick dim **QS** (reading dims is key)
- [ ] 41. Dimension edit, break, space, override
- [ ] 42. Leaders and multileaders
- [ ] 43. Tables **QS**
- [ ] 44. Fields (auto-updating text)
- [ ] 45. Geometric tolerance
- [ ] 46. Annotation scale

## 6. Layers

- [ ] 47. Layer panel: create, rename, colour, linetype, lock, freeze, on/off **QS** (filter by discipline)
- [ ] 48. Layer isolate, un-isolate, match layer, make current
- [ ] 49. Layer states, layer translator
- [ ] 50. Viewport layer overrides

## 7. Blocks and content

- [ ] 51. Insert block, create block, edit block in place, write block to file **QS** (blocks = countable items)
- [ ] 52. Block attributes: define, edit, manage
- [ ] 53. Block palette and content browser
- [ ] 54. Design centre (browse blocks across drawings)
- [ ] 55. Tool palettes

## 8. View and navigation

- [ ] 56. Zoom (extents, window, in, out), pan **QS**
- [ ] 57. Orbit, perspective and orthographic cameras, view cube **CAD**
- [ ] 58. Named views, standard views (top, front, iso)
- [ ] 59. Visual styles (wireframe, shaded)
- [ ] 60. Multiple viewports, tile and cascade windows
- [ ] 61. Model space and paper space layouts, layout tabs **QS** (sheets live in layouts)
- [ ] 62. User coordinate systems, drawing limits
- [ ] 63. Sheet set manager

## 9. 3D modelling

- [ ] 64. Primitives: box, cylinder, sphere, polysolid **CAD**
- [ ] 65. Extrude, revolve, sweep, loft **CAD**
- [ ] 66. Boolean union, subtract, intersect **CAD**
- [ ] 67. Slice, shell, section plane, edge tools **CAD**
- [ ] 68. Mesh and 3D face **CAD**

## 10. Parametric constraints

- [ ] 69. Geometric constraints: coincident, concentric, tangent, perpendicular, horizontal, equal, symmetric, fixed, smooth **CAD**
- [ ] 70. Dimensional constraints and a parameters manager **CAD**
- [ ] 71. Auto-constrain settings **CAD**

## 11. Drafting aids

- [ ] 72. Object snaps (end, mid, centre, intersection, and more) **QS**
- [ ] 73. Polar and object tracking, ortho mode
- [ ] 74. Grid and snap, drafting settings, drawing units
- [ ] 75. Properties palette **QS**
- [ ] 76. Match properties
- [ ] 77. Command line with aliases and shortcuts
- [ ] 78. Ribbon, side toolbar, status bar, docking panels, themes

## 12. Automation and extension

- [ ] 79. Command scripts (SCR files)
- [ ] 80. Headless conversion from the command line
- [ ] 81. JSON automation API over a socket (an AI agent can drive it) **QS**
- [ ] 82. MCP server for AI agents **QS**
- [ ] 83. Native plugins and a Python plugin
- [ ] 84. Audit, purge, options, customise the user interface

## 13. Platform

- [ ] 85. Desktop: Windows, Linux, macOS
- [ ] 86. Web app in the browser (same core, WebAssembly)
- [ ] 87. 22 interface languages, including Arabic
- [ ] 88. Update check, single instance, crash recovery

## My view

Everything with a **QS** tag is a small fraction of the code but most of the value for our users.
A first version that opens DWG, DXF and PDF, lets you measure length, area and count, filters by layer, and sends the numbers to a BOQ is a real product.
Drawing, 3D and parametric tools are where the 530,000 lines live. We should only take them if we decide to compete with CAD itself.
