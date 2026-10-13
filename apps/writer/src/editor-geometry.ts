// Reflow at the visible pane width; hidden checkpoints retain native coordinates.
export function nativeViewport(cssWidth:number,nativeWidth:number,scrollTop:number,height:number,anchor:number,displayedPointerWidth?:number){
  const visible=Number.isFinite(cssWidth)&&cssWidth>0;
  const previous=Number.isFinite(nativeWidth)&&nativeWidth>0?nativeWidth:640;
  const requested=displayedPointerWidth??(visible?cssWidth:previous);
  const width=Math.max(240,Math.min(4096,Math.round(Number.isFinite(requested)&&requested>0?requested:previous)));
  const scale=visible?cssWidth/width:1;
  // The backend maps this old-layout passage through width reflow.
  const previousScale=visible?cssWidth/previous:1;
  const top=visible?scrollTop/previousScale:anchor;
  return {width,scale,top:Math.max(0,Math.round(Number.isFinite(top)?top:0)),viewportHeight:Math.max(120,Math.min(2160,Math.ceil((Number.isFinite(height)&&height>0?height:620)/scale)))};
}

/** Mouse positions are measured against a displayed frame, not a later resize. */
export function editorRequestViewport(operation:Record<string,unknown>,live:ReturnType<typeof nativeViewport>){
  if(operation.type==='mouse'&&typeof operation.width==='number'&&typeof operation.top==='number'&&typeof operation.viewportHeight==='number')return {width:operation.width,top:operation.top,viewportHeight:operation.viewportHeight};
  return {width:live.width,top:live.top,viewportHeight:live.viewportHeight};
}
