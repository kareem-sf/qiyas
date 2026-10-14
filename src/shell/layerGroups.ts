/** The layers panel's tree: a drawing with many layers is grouped by the discipline prefix its
 *  names share (`A-` for A-WALL, A-DOOR; `S-`, `E-`...) or, for an xref's layers, by the xref
 *  (`PLAN|A-DOOR` is under PLAN). A short list stays flat, and so does a layer with no group of
 *  two. Pure, so it is tested on its own. */
import type { LayerInfo } from "../api";

/** Fewer layers than this read better as one list. */
export const GROUP_FROM = 12;

export type LayerNode =
  | { id: string; kind: "layer"; name: string; layer: LayerInfo }
  | { id: string; kind: "group"; name: string; children: string[] };

export type LayerTree = {
  /** Every node by id, the root (`ROOT`) included. */
  nodes: Record<string, LayerNode & { children?: string[] }>;
  /** The root's children, in the order the layers come. */
  root: string[];
  /** The ids of the groups. */
  groups: string[];
};

export const ROOT = "root";
export const layerId = (name: string) => `layer:${name}`;
export const groupId = (key: string) => `group:${key}`;

/** The group a layer name belongs to, or null: an xref's name before the `|`, or the one or two
 *  letters before the first `-` (the AIA discipline designator), in capitals. */
export function groupKey(name: string): string | null {
  const bar = name.indexOf("|");
  if (bar > 0) return name.slice(0, bar);
  const prefix = /^([A-Za-z]{1,2})-/.exec(name);
  return prefix ? `${prefix[1].toUpperCase()}-` : null;
}

export function buildLayerTree(layers: LayerInfo[]): LayerTree {
  const nodes: LayerTree["nodes"] = {};
  const root: string[] = [];
  const groups: string[] = [];
  const keys = layers.map((l) => (layers.length >= GROUP_FROM ? groupKey(l.name) : null));
  const size = new Map<string, number>();
  for (const key of keys) if (key !== null) size.set(key, (size.get(key) ?? 0) + 1);
  layers.forEach((layer, i) => {
    const id = layerId(layer.name);
    nodes[id] = { id, kind: "layer", name: layer.name, layer };
    const key = keys[i];
    if (key === null || (size.get(key) ?? 0) < 2) {
      root.push(id);
      return;
    }
    const gid = groupId(key);
    if (!nodes[gid]) {
      nodes[gid] = { id: gid, kind: "group", name: key, children: [] };
      root.push(gid);
      groups.push(gid);
    }
    (nodes[gid] as Extract<LayerNode, { kind: "group" }>).children.push(id);
  });
  nodes[ROOT] = { id: ROOT, kind: "group", name: "", children: root };
  return { nodes, root, groups };
}
