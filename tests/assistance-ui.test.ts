import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe,expect,it } from 'vitest';
import { AssistanceWorkspace } from '../apps/writer/src/assistance/AssistanceWorkspace';
import { anchorFor,type SceneSnapshot,type Suggestion } from '../apps/writer/src/assistance/model';
const scene: SceneSnapshot = { sceneId:'s1',revisionId:'r1',hash:'a'.repeat(64),title:'Synthetic scene',text:'Mira walked home.',language:'en' };
function suggestion():Suggestion { return { id:'suggestion',origin:'llm',title:'Check wording',explanation:'Local proposal.',sources:[scene],evidence:[anchorFor(scene,5,11)],action:{ kind:'replace',replacement:'ran' },status:'pending' }; }
function render(suggestions:Suggestion[],current:SceneSnapshot|null = scene) { return renderToStaticMarkup(React.createElement(AssistanceWorkspace,{ projectId:'p1',scene:current,suggestions,onApplyPatch:async () => ({ revisionId:'r2',hash:'b'.repeat(64) }) })); }
describe('assistance review presentation',() => {
  it('shows the measured experimental-model limitation before review decisions',()=>{const html=render([suggestion()]);expect(html).toContain('Experimental AI');expect(html).toContain('Qwen3 flagged all four clean test scenes');expect(render([])).not.toContain('Experimental AI');});
  it('shows quoted evidence, proposed replacement and explicit decision controls',() => { const html = render([suggestion()]); expect(html).toContain('Local model · provisional'); expect(html).toContain('<mark>walked</mark>'); expect(html).toContain('Proposed replacement'); expect(html).toContain('Accept replacement'); expect(html).toContain('Reject'); expect(html).toContain('Preview the resulting scene'); expect(html).toContain('New review'); expect(html).toContain('Source revisions'); expect(html).toContain('r1'); });
  it('marks stale sources and disables the accept control',() => { const html = render([suggestion()],{ ...scene,revisionId:'r2' }); expect(html).toContain('Stale'); expect(html).toMatch(/<button disabled=""[^>]*>Accept replacement/); expect(html).not.toContain('Preview the resulting scene'); });
  it('requires a separate fact approval confirmation',() => { const html = render([{ ...suggestion(),action:{ kind:'fact',subject:'Mira',property:'occupation',value:'navigator' } }]); expect(html).toContain('I confirm this as a story-world fact'); expect(html).toMatch(/<button disabled=""[^>]*>Approve fact/); expect(html).not.toContain('Accept replacement'); });
  it('escapes untrusted review text rather than rendering model HTML',() => { const html = render([{ ...suggestion(),title:'<script>alert(1)</script>',explanation:'<img src="https://example.test/tracker">' }]); expect(html).toContain('&lt;script&gt;'); expect(html).not.toContain('<script>'); expect(html).not.toContain('<img src='); });
  it('presents read-only stale proposals when no saved scene is open',() => { const html = render([suggestion()],null); expect(html).toContain('Open and save a scene'); expect(html).toMatch(/<button disabled=""[^>]*>Accept replacement/); });
});
