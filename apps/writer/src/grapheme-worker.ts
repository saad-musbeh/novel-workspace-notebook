import {graphemeCount} from './document';
self.onmessage=(event:MessageEvent<{id:number;text:string}>)=>{
 const {id,text}=event.data;
 self.postMessage({id,count:graphemeCount(text)});
};
