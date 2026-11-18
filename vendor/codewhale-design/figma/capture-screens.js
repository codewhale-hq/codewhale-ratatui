const page = figma.root.children.find(node => node.id === '229:6');
if (!page) throw Error('Screens page missing');
await figma.setCurrentPageAsync(page);
const ids = typeof CW_CAPTURE_IDS === 'undefined' ? ['240:9', '241:550', '241:617', '242:1255', '243:1344'] : CW_CAPTURE_IDS;
if (!Array.isArray(ids) || ids.length > 5) throw Error('Capture at most five reviewed roots');
const output = [];
const offset = typeof CW_CAPTURE_OFFSET === 'undefined' ? 0 : CW_CAPTURE_OFFSET;
const count = typeof CW_CAPTURE_COUNT === 'undefined' ? 50 : CW_CAPTURE_COUNT;
if (!Number.isInteger(offset) || offset < 0 || !Number.isInteger(count) || count < 1 || count > 100) throw Error('Invalid capture chunk');
const fields = ['id', 'type', 'name', 'x', 'y', 'width', 'height', 'visible', 'layoutMode', 'itemSpacing', 'paddingLeft', 'paddingRight', 'paddingTop', 'paddingBottom', 'fills', 'strokes', 'cornerRadius', 'characters', 'fontName', 'fontSize', 'lineHeight'];
for (const id of ids) {
  const root = await figma.getNodeByIdAsync(id);
  let ancestor = root;
  while (ancestor && ancestor.type !== 'PAGE') ancestor = ancestor.parent;
  if (!root || ancestor?.id !== page.id) throw Error('Root moved or missing: ' + id);
  const nodes = [root, ...root.findAll()];
  if (nodes.length > 2500) throw Error('Root exceeds capture bound: ' + id);
  const records = [];
  for (const node of nodes.slice(offset, offset + count)) {
    const record = { parentId: node.parent?.id ?? null };
    for (const field of fields) if (field in node) record[field] = node[field] === figma.mixed ? { mixed: true } : node[field];
    if (node.type === 'INSTANCE') record.mainComponentId = (await node.getMainComponentAsync())?.id ?? null;
    if (node.type === 'TEXT') record.segments = node.getStyledTextSegments(['fontName', 'fontSize', 'fills']);
    records.push(record);
  }
  output.push({ rootId: id, totalNodes: nodes.length, offset, nextOffset: offset + records.length < nodes.length ? offset + records.length : null, nodes: records });
  if (JSON.stringify(output).length > 100000) throw Error('Capture exceeds output bound; reduce CW_CAPTURE_COUNT');
}
return { pageId: page.id, readOnly: true, createdNodeIds: [], mutatedNodeIds: [], roots: output };
