import React from 'react';
import {invoke} from '@tauri-apps/api/core';
import {plainText,type DocumentNode} from './document';
export class StartupBoundary extends React.Component<{children:React.ReactNode},{error:string;notice:string}>{
 state={error:'',notice:''};
 static getDerivedStateFromError(error:unknown){return {error:String(error)};}
 async recover(){try{const frame=await invoke<{document:DocumentNode}>('editor_edit',{operation:{type:'snapshot'}});if(!frame.document)throw new Error('The editor has no recoverable document yet.');await invoke('export_document',{format:'text',content:plainText(frame.document),suggestedName:'editor-recovery.txt'});this.setState({notice:'The editor recovery text was offered for export. Existing saved revisions remain on disk.'});}catch(e){this.setState({notice:String(e)});}}
 render(){if(!this.state.error)return this.props.children;return <section className="load-error" role="alert"><h1>The workspace could not display this view</h1><p>{this.state.error}</p><p>Your acknowledged revisions remain on disk.</p><button onClick={()=>void this.recover()}>Export editor recovery text</button><p role="status">{this.state.notice}</p></section>;}
}
