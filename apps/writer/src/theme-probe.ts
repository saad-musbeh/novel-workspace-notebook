/** Native synthetic regression probe; reads the actual WebKit computed palette. */
export function checkThemeContrast():void {
 const root=document.documentElement,previous=root.dataset.theme;
 // The isolated probe may have no incoming changes. Exercise the actual notice
 // palette, including disabled text, without mutating any manuscript state.
 const notice=document.createElement('p');notice.className='workspace-notice';notice.textContent='Synthetic shared change. ';
 const refresh=document.createElement('button');refresh.textContent='Refresh';
 const disabled=document.createElement('button');disabled.textContent='Refreshing';disabled.disabled=true;
 notice.append(refresh,disabled);document.body.append(notice);
 const disclosures=[...document.querySelectorAll<HTMLDetailsElement>('details')].filter(e=>!e.closest('[hidden]')).map(e=>({element:e,open:e.open}));
 for(const {element} of disclosures)element.open=true;
 const rgb=(value:string)=>{const channels=value.match(/[\d.]+/g)?.map(Number);if(!channels||channels.length<3)throw new Error('Unsupported computed theme color: '+value);return channels;};
 const luminance=(channels:number[])=>channels.slice(0,3).map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;}).reduce((sum,v,i)=>sum+v*[.2126,.7152,.0722][i],0);
 const selectors=['.workspace-nav button','.page-actions button','.page-menu summary','.manuscript-tree button','.toolbar button','.paragraph-options summary','.paragraph-settings label','.paragraph-settings select','.editor-footer','.planning-save','.planning-tabs button','.planning-panel','.planning-help summary','.ws-heading>span','.ws-body','.assistance label','.assistance-provider','.assistance-card','.assistance-muted','.assistance summary','.workspace-panel summary','.workspace-panel label','.workspace-panel input','.workspace-panel select','.workspace-notice','.workspace-notice button'];
 try {for(const theme of ['light','dark','system']){root.dataset.theme=theme;for(const selector of selectors){for(const element of document.querySelectorAll<HTMLElement>(selector)){const inNotice=element.closest('.workspace-notice');if(!element.getClientRects().length||(element.matches(':disabled')&&!inNotice))continue;if(inNotice&&getComputedStyle(element).opacity!=='1')throw new Error(theme+' notice text must remain opaque');const foreground=rgb(getComputedStyle(element).color);let ancestor:Element|null=element;const layers:number[][]=[];while(ancestor){const candidate=rgb(getComputedStyle(ancestor).backgroundColor);layers.push(candidate);if(candidate.length===3||candidate[3]===1)break;ancestor=ancestor.parentElement;}if(!ancestor)throw new Error('Missing opaque theme background for '+selector);let background=[255,255,255];for(const layer of layers.reverse()){const alpha=layer[3]??1;background=background.map((v,i)=>layer[i]*alpha+v*(1-alpha));}const a=luminance(foreground),b=luminance(background),ratio=(Math.max(a,b)+.05)/(Math.min(a,b)+.05);if(ratio<4.5)throw new Error(`${theme} ${selector}: text contrast ${ratio.toFixed(2)} is below 4.5 (${getComputedStyle(element).color} on ${background}; ${element.className}; active=${element.getAttribute('aria-pressed')})`);}}}}
 finally {notice.remove();for(const {element,open} of disclosures)element.open=open;if(previous===undefined)delete root.dataset.theme;else root.dataset.theme=previous;}
}

/** Read-only snapshots of the isolated synthetic workspace for visual inspection. */
export function captureWorkspaceForProbe():string {
 const app=document.querySelector('.app')!.cloneNode(true) as HTMLElement;
 app.querySelectorAll('[hidden]').forEach(element=>element.remove());
 app.removeAttribute('inert');
 return app.outerHTML;
}
