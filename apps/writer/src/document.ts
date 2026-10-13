// The stored scene contract is independent of a particular editor package.
export interface DocumentNode {
  type?: string;
  text?: string;
  attrs?: Record<string, unknown>;
  marks?: { type: string }[];
  content?: DocumentNode[];
}
export type Direction = 'auto' | 'ltr' | 'rtl';
export const lineSpacingValues = [1, 1.15, 1.5, 2] as const;
export type LineSpacing = typeof lineSpacingValues[number];
export interface Scene { schemaVersion: 1; title: string; document: DocumentNode }
export interface Snapshot { revisionId: string | null; scene: Scene }
export const initialScene: Scene = {
  schemaVersion: 1, title: 'First light', document: { type: 'doc', content: [
    { type: 'paragraph', attrs: { dir: 'auto', lang: 'en' }, content: [{ type: 'text', text: 'Every story begins with a small possibility. This one is yours.' }] },
    { type: 'paragraph', attrs: { dir: 'rtl', lang: 'ar' }, content: [{ type: 'text', text: 'تبدأ الحكاية بخطوة صغيرة، ثم تجد الكلمات طريقها.' }] },
    { type: 'paragraph', attrs: { dir: 'auto', lang: 'zh-Hans' }, content: [{ type: 'text', text: '故事从这里开始。每一个字，都通向一个新的世界。' }] },
    { type: 'paragraph', attrs: { dir: 'auto', lang: 'zh-Hant' }, content: [{ type: 'text', text: '故事從這裡開始。' }] },
  ] },
};
export function validateScene(scene: Scene): void {
  if (scene.schemaVersion !== 1) throw new Error('Unsupported document version. This scene has not been changed.');
  if (typeof scene.title !== 'string' || scene.title.length > 500) throw new Error('Scene title is invalid.');
  if (JSON.stringify(scene).length > 2_000_000) throw new Error('This prototype supports scenes up to 2 MB.');
  function check(node: DocumentNode, depth: number): void {
    if (depth > 8 || !['doc', 'paragraph', 'heading', 'text', 'hardBreak'].includes(node.type ?? '')) throw new Error('Unsupported document content.');
    if (node.type === 'text' && typeof node.text !== 'string') throw new Error('Invalid text.');
    if (node.attrs?.dir !== undefined && (typeof node.attrs.dir !== 'string' || !['auto','ltr','rtl'].includes(node.attrs.dir))) throw new Error('Invalid paragraph direction.');
    if (node.attrs?.lang !== undefined && (typeof node.attrs.lang !== 'string' || !['en','ar','zh-Hans','zh-Hant','und'].includes(node.attrs.lang))) throw new Error('Unsupported paragraph language.');
    if (node.attrs?.lineSpacing !== undefined && (!['paragraph', 'heading'].includes(node.type ?? '') || typeof node.attrs.lineSpacing !== 'number' || !lineSpacingValues.some(value => value === node.attrs?.lineSpacing))) throw new Error('Unsupported line spacing.');
    if (node.marks?.some(m => !['bold','italic','underline'].includes(m.type))) throw new Error('Unsupported formatting.');
    node.content?.forEach(child => check(child, depth + 1));
  }
  if (scene.document.type !== 'doc') throw new Error('Missing document root.');
  check(scene.document, 0);
}
export function plainText(node: DocumentNode): string {
  if (node.type === 'text') return node.text ?? '';
  if (node.type === 'hardBreak') return '\n';
  return (node.content ?? []).map(plainText).join(node.type === 'doc' ? '\n\n' : '');
}
export function graphemeCount(text: string): number {
  let count=0;
  for(const _ of new Intl.Segmenter(undefined,{granularity:'grapheme'}).segment(text))count++;
  return count;
}
