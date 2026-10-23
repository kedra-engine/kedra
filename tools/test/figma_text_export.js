/* Explicit Figma Plugin API authoring entry point. No network, node creation beyond
 * one disposable clone, image processing or native-capture inputs. */
async function exportTextScenario(figma, request) {
  const source=await figma.getNodeByIdAsync(request.node);
  if(!source||source.type!=='FRAME')throw Error('Expected an exact source export frame');
  const visible=node=>{
    for(let p=node;p&&p!==source.parent;p=p.parent)if(p.visible===false||p.opacity===0)return false;
    return true;
  };
  const texts=source.findAllWithCriteria({types:['TEXT']});
  const roles={};
  for(const text of texts.filter(visible)) {
    const role=text.componentPropertyReferences?.characters;
    if(!request.roles.includes(role)||roles[role])throw Error('Missing, extra or duplicate text role property');
    const paints=Array.isArray(text.fills)?text.fills.filter(p=>p.visible!==false):[];
    if(paints.length!==1||paints[0].type!=='SOLID'||!Number.isFinite(text.fontSize)
      ||!Number.isFinite(text.fontWeight)||text.lineHeight?.unit!=='PIXELS'
      ||text.letterSpacing?.unit!=='PIXELS'||!text.fontName?.family
      ||text.effects?.some(e=>e.visible!==false)||text.strokes?.some(p=>p.visible!==false))
      throw Error('Unsupported non-uniform source text style');
    const t=text.absoluteTransform,f=source.absoluteTransform;
    if([t,f].some(m=>m[0][0]!==1||m[0][1]!==0||m[1][0]!==0||m[1][1]!==1))
      throw Error('Unsupported transformed source text');
    const x=t[0][2]-f[0][2],y=t[1][2]-f[1][2];
    let opacity=paints[0].opacity??1;
    for(let p=text;p&&p!==source.parent;p=p.parent)opacity*=p.opacity??1;
    const c=paints[0].color;
    roles[role]={node:text.id,text:text.characters,font_family:text.fontName.family,font_style:text.fontName.style,
      size_px:text.fontSize,weight:text.fontWeight,line_height_px:text.lineHeight.value,
      letter_spacing_px:text.letterSpacing.value,color:[c.r,c.g,c.b,opacity].map(v=>Math.round(v*255)),
      rect:[x,y,x+text.width,y+text.height],lines:text.characters.split('\n').length};
  }
  if(Object.keys(roles).length!==request.roles.length)throw Error('Expected source text role is missing');
  const instances=[];
  for(const instance of source.children.filter(n=>n.type==='INSTANCE')){
    const component=await instance.getMainComponentAsync();
    if(!component)throw Error('Source canonical component is missing');
    instances.push({node:instance.id,component:component.id,key:component.key,properties:instance.componentProperties});
  }
  const metadata={schema:1,exporter:'kedra-text-source-v1',case:request.case,node:source.id,
    file_key:figma.fileKey,size:[source.width,source.height],region:request.region,
    roles,instances,shape_derivation:'Figma temporary clone; text opacity zero; layout retained'};
  const partial={metadata};
  const createdNodeIds=[];
  let copy;
  try {
    const full=await source.exportAsync({format:'PNG',constraint:{type:'SCALE',value:1}});
    partial.full=full;
    // Existing fonts must be loaded even when only changing a text node's opacity.
    for(const text of texts)for(const segment of text.getStyledTextSegments(['fontName']))
      await figma.loadFontAsync(segment.fontName);
    copy=source.clone();createdNodeIds.push(copy.id);
    // Materialize direct instance descendants before asking the host's index.
    // In-scope QA wrappers own a canonical instance; ordinary Plugin API eagerly
    // has these children, while the MCP host exposes them lazily after cloning.
    for(const child of copy.children)if(child.type==='INSTANCE')void child.children;
    // The fast criteria index can lag behind freshly cloned instance descendants.
    // Standard findAll traverses the clone's current children instead of that index.
    const descendants=copy.findAll(()=>true);
    createdNodeIds.push(...descendants.map(n=>n.id));
    const copyTexts=descendants.filter(n=>n.type==='TEXT');
    if(copyTexts.length!==texts.length)throw Error('Clone text inventory differs from independent source');
    for(const text of copyTexts)text.opacity=0;
    const shape=await copy.exportAsync({format:'PNG',constraint:{type:'SCALE',value:1}});
    return {full,shape,createdNodeIds,removedNodeIds:createdNodeIds,metadata};
  } catch(caught) {
    const error=caught instanceof Error?caught:Error(String(caught));
    error.partial=partial;throw error;
  } finally {
    if(copy)copy.remove();
    partial.removedNodeIds=createdNodeIds;
  }
}
