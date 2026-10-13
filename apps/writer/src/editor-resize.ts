export const EDITOR_RESIZE_EVENT='novel-editor-resize';
/** Observe real layout changes and explicit post-commit notifications, with no polling. */
export function observeEditorSize(surface:HTMLElement,viewport:HTMLElement|null,onChange:(width:number,height:number)=>void,target:EventTarget=window){
 let previousWidth=0,previousHeight=0,disposed=false;
 const measure=()=>{
  if(disposed)return;
  const width=surface.clientWidth,height=viewport?.clientHeight??0;
  if(!Number.isFinite(width)||!Number.isFinite(height)||width<=0||height<=0||(width===previousWidth&&height===previousHeight))return;
  previousWidth=width;previousHeight=height;onChange(width,height);
 };
 const observer=typeof ResizeObserver==='undefined'?null:new ResizeObserver(measure);
 observer?.observe(surface);if(viewport)observer?.observe(viewport);
 target.addEventListener('resize',measure);target.addEventListener(EDITOR_RESIZE_EVENT,measure);
 measure();
 return {measure,dispose(){disposed=true;observer?.disconnect();target.removeEventListener('resize',measure);target.removeEventListener(EDITOR_RESIZE_EVENT,measure);}};
}
