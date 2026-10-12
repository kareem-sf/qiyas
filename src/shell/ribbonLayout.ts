/** The ribbon as data: the layout the window draws when the user has none of their own
 *  (`DEFAULT_RIBBON`), and the Customize dialog's tree of it, where tabs, groups and buttons can
 *  be hidden and moved. The core keeps the default too (`get_default_ribbon`) and checks a saved
 *  layout against it: a saved layout may only hold the core default's buttons. */
import type { RibbonButton, RibbonButtonKind, RibbonGroup, RibbonLayout, RibbonTab } from "../api";

const button = (kind: RibbonButtonKind, command: string, label: string): RibbonButton => ({ kind, command, label });
const tool = (command: string, label = command) => button("tool", command, label);
const action = (command: string, label = command) => button("action", command, label);
const toggle = (command: string, label = command) => button("toggle", command, label);
/** Tools labelled `<prefix>.<name>`. */
const tools = (prefix: string, names: string[]) => names.map((n) => tool(n, `${prefix}.${n}`));
const group = (id: string, label: string, buttons: RibbonButton[]): RibbonGroup => ({ id, label, buttons });
const tab = (id: string, groups: RibbonGroup[]): RibbonTab => ({ id, label: id, groups });

/** The ribbon Qiyas ships: the same as the core's `default_ribbon` (`qiyas-desktop/src/ribbon.rs`);
 *  `ribbon.default.json` pins both (`ribbonLayout.test.ts` and the core's `tests/options.rs`). */
export const DEFAULT_RIBBON: RibbonLayout = {
  tabs: [
    tab("home", [
      group("file", "ribbon.file", [
        action("open"),
        action("save"),
        action("saveAs"),
        action("recover", "recover.button"),
        action("exportTakeoff"),
        action("importBoq"),
        action("compare"),
        action("exportConflicts"),
      ]),
      group("history", "ribbon.history", [action("undo"), action("redo")]),
      group(
        "draw",
        "draw.group",
        tools("draw", ["line", "polyline", "rectangle", "circle", "arc", "ellipse", "spline", "point", "hatch", "revcloud", "wipeout"]),
      ),
      group("modify", "modify.group", [
        ...tools("modify", ["move", "copy", "rotate", "scale", "mirror", "stretch", "align", "array", "explode", "erase"]),
        ...tools("edit", [
          "trim",
          "extend",
          "break",
          "join",
          "lengthen",
          "reverse",
          "offset",
          "fillet",
          "chamfer",
          "blend",
          "pedit",
          "overkill",
          "flatten",
        ]),
      ]),
      group("clipboard", "clip.group", tools("clip", ["copyclip", "copybase", "pasteclip", "pasteblock"])),
      group("text", "text.group", [...tools("text", ["text", "mtext", "textEdit"]), action("find", "text.find")]),
      group("layers", "layers", [
        action("layerStates", "layer.statesButton"),
        action("translateLayers", "layer.translateButton"),
        ...tools("layerTool", ["group", "ungroup", "matchprop"]),
        toggle("groupSelect", "group.select"),
      ]),
    ]),
    tab("measure", [
      group("measure", "ribbon.measure", ["select", "distance", "area", "count"].map((n) => tool(n))),
      group("inquiry", "ribbon.inquiry", [
        ...["geometry", "divide", "measureAlong", "idPoint"].map((n) => tool(n)),
        action("extract"),
      ]),
    ]),
    tab("annotate", [
      group("dims", "dim.group", [
        ...tools("dim", [
          "dimLinear",
          "dimAligned",
          "dimAngular",
          "dimRadius",
          "dimDiameter",
          "dimArc",
          "dimOrdinate",
          "dimBaseline",
          "dimContinue",
          "qdim",
          "dimEdit",
          "dimBreak",
          "dimSpace",
          "dimJogLine",
        ]),
        action("dimStyle", "dim.styleButton"),
      ]),
      group("notes", "dim.notes", tools("dim", ["leader", "mleader", "tolerance"])),
      group("tables", "table.group", [
        tool("table", "table.table"),
        tool("tableEdit", "table.tableEdit"),
        tool("field", "table.fieldTool"),
        action("updateFields", "table.updateFields"),
      ]),
    ]),
    tab("insert", [
      group("block", "block.group", [
        tool("block", "block.createTool"),
        action("blockPalette", "block.insertTool"),
        action("writeBlock", "block.writeTool"),
        tool("refedit", "block.refedit"),
        tool("attdef", "block.attdefTool"),
        tool("textEdit", "block.attedit"),
        action("attributeManager", "block.manager"),
        action("designCenter", "block.designCenter"),
        action("toolPalettes", "block.toolPalettes"),
      ]),
    ]),
    tab("view", [
      group("views", "ribbon.views", [
        action("zoomExtents"),
        tool("zoomWindow"),
        action("views"),
        action("xrefs"),
        action("attachPdf"),
        action("attachImage"),
        action("calibrate"),
      ]),
      group("vports", "vport.group", [
        tool("mview", "vport.mview"),
        action("split", "vport.two"),
        tool("ucs", "ucs.tool"),
        tool("ucsObject", "ucs.object"),
        action("ucsWorld", "ucs.world"),
        action("sheetSet", "sheetSet.button"),
      ]),
      group("display", "ribbon.display", [toggle("arabicReversed")]),
    ]),
    tab("manage", [
      group("scripts", "ribbon.scripts", [action("runScript", "script.run")]),
      group("drawing", "ribbon.drawing", [action("audit", "audit.button"), action("purge", "purge.button")]),
      group("setup", "ribbon.setup", [
        action("options", "options.button"),
        action("customize", "cui.button"),
        action("aliases", "aliasEdit.button"),
      ]),
    ]),
    tab("output", [
      group("plot", "ribbon.plot", [
        action("plot", "plot.button"),
        action("quickPrint", "plot.quickPrint"),
        action("printAll", "plot.printAll"),
      ]),
      group("plotStyles", "ribbon.plotStyles", [action("plotStyles", "plot.styles")]),
    ]),
  ],
};

/** A button's identity: what it runs (a layout may hold one twice). */
export const buttonKey = (b: Pick<RibbonButton, "kind" | "command">) => `${b.kind}:${b.command}`;

/** The buttons a layout holds, by `buttonKey`. */
export const buttonKeys = (layout: RibbonLayout) =>
  new Set(layout.tabs.flatMap((t) => t.groups.flatMap((g) => g.buttons.map(buttonKey))));

export type EditButton = RibbonButton & { hidden: boolean };
export type EditGroup = Omit<RibbonGroup, "buttons"> & { hidden: boolean; buttons: EditButton[] };
export type EditTab = Omit<RibbonTab, "groups"> & { hidden: boolean; groups: EditGroup[] };

/** The Customize dialog's tree: what is on the ribbon (`saved`, or the window's default when
 *  there is none, less the buttons `all` lacks), in its order, then what `all` (the core's
 *  default) has that it leaves out, hidden, where `all` keeps it. */
export function editTree(all: RibbonLayout, saved: RibbonLayout | null): EditTab[] {
  const known = buttonKeys(all);
  const shown = saved ?? DEFAULT_RIBBON;
  const tree: EditTab[] = shown.tabs.map((t) => ({
    ...t,
    hidden: false,
    groups: t.groups.map((g) => ({
      ...g,
      hidden: false,
      buttons: g.buttons.filter((b) => known.has(buttonKey(b))).map((b) => ({ ...b, hidden: false })),
    })),
  }));
  for (const t of all.tabs) {
    const mine = tree.find((x) => x.id === t.id);
    if (!mine) {
      tree.push({
        ...t,
        hidden: true,
        groups: t.groups.map((g) => ({ ...g, hidden: false, buttons: g.buttons.map((b) => ({ ...b, hidden: false })) })),
      });
      continue;
    }
    for (const g of t.groups) {
      const theirs = mine.groups.find((x) => x.id === g.id);
      if (!theirs) {
        mine.groups.push({ ...g, hidden: true, buttons: g.buttons.map((b) => ({ ...b, hidden: false })) });
        continue;
      }
      const have = new Set(theirs.buttons.map(buttonKey));
      for (const b of g.buttons) if (!have.has(buttonKey(b))) theirs.buttons.push({ ...b, hidden: true });
    }
  }
  return tree;
}

/** The layout a tree saves: what is not hidden, without groups left empty or tabs left with no
 *  group; null when no tab is left. */
export function layoutOf(tree: EditTab[]): RibbonLayout | null {
  const tabs = tree
    .filter((t) => !t.hidden)
    .map(({ id, label, groups }) => ({
      id,
      label,
      groups: groups
        .filter((g) => !g.hidden)
        .map(({ id, label, buttons }) => ({
          id,
          label,
          buttons: buttons.filter((b) => !b.hidden).map(({ kind, command, label }) => ({ kind, command, label })),
        }))
        .filter((g) => g.buttons.length > 0),
    }))
    .filter((t) => t.groups.length > 0);
  return tabs.length > 0 ? { tabs } : null;
}
