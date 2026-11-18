const input = typeof CW_NODE_PATCHES === 'undefined' ? null : CW_NODE_PATCHES;
if (!input || !Array.isArray(input.nodes) || typeof input.pageId !== 'string') throw Error('Supply reviewed CW_NODE_PATCHES with one pageId and exact node IDs.');
const apply = typeof APPLY_PATCHES !== 'undefined' && APPLY_PATCHES === true;
const page = figma.root.children.find(node => node.id === input.pageId);
if (!page || page.type !== 'PAGE') throw Error('Reviewed page missing');
await figma.setCurrentPageAsync(page);
const allowed = new Set(['name', 'description', 'fills', 'strokes', 'characters']);
const prepared = [];
const conflicts = [];
const seen = new Set();
for (const patch of input.nodes) {
  if (seen.has(patch.id)) throw Error('Duplicate node patch: ' + patch.id);
  seen.add(patch.id);
  const node = await figma.getNodeByIdAsync(patch.id);
  let ancestor = node;
  while (ancestor && ancestor.type !== 'PAGE') ancestor = ancestor.parent;
  if (!node || ancestor?.id !== page.id || node.type !== patch.type) throw Error('Node identity/page/type changed: ' + patch.id);
  const changes = [];
  const observed = [];
  for (const [key, value] of Object.entries(patch.after)) {
    if (!allowed.has(key) || !(key in node)) throw Error('Unsupported property ' + key);
    if (key === 'description' && !['COMPONENT', 'COMPONENT_SET'].includes(node.type)) throw Error('Description requires a component');
    if (key === 'characters' && node.type !== 'TEXT') throw Error('Characters requires text');
    const current = JSON.stringify(node[key]);
    observed.push([key, current]);
    if (current === JSON.stringify(value)) continue;
    if (!Object.hasOwn(patch.before, key) || current !== JSON.stringify(patch.before[key])) conflicts.push({ id: node.id, key });
    changes.push([key, value]);
  }
  if (node.type === 'TEXT') {
    if (node.characters.length) {
      for (const segment of node.getStyledTextSegments(['fontName'])) await figma.loadFontAsync(segment.fontName);
    } else if (node.fontName !== figma.mixed) await figma.loadFontAsync(node.fontName);
    else throw Error('Empty mixed-font node requires inspection');
  }
  prepared.push({ node, changes, observed, id: patch.id, type: patch.type });
}
for (const entry of prepared) {
  const { node } = entry;
  if (node.removed) { conflicts.push({ id: entry.id, key: 'removed' }); continue; }
  let ancestor = node;
  while (ancestor && ancestor.type !== 'PAGE') ancestor = ancestor.parent;
  if (node.id !== entry.id || node.type !== entry.type || ancestor?.id !== page.id) conflicts.push({ id: entry.id, key: 'identity' });
  for (const [key, value] of entry.observed) if (JSON.stringify(node[key]) !== value) conflicts.push({ id: entry.id, key });
}
if (conflicts.length) return { dryRun: !apply, conflicts, mutatedNodeIds: [], createdNodeIds: [], safeToRetryWithoutCanvasRead: false };
if (apply) for (const { node, changes } of prepared) for (const [key, value] of changes) node[key] = value;
return { dryRun: !apply, pageId: page.id, checkedNodeIds: prepared.map(entry => entry.node.id), mutatedNodeIds: apply ? prepared.filter(entry => entry.changes.length).map(entry => entry.node.id) : [], createdNodeIds: [], planned: prepared.map(entry => ({ id: entry.node.id, fields: entry.changes.map(([key]) => key) })), geometryChanged: false, instancesDetached: false, safeToRetryWithoutCanvasRead: true };
