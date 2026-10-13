export interface QueuedOperation {
  operation: Record<string, unknown>;
  tickets: number[];
  edited: boolean;
  epoch: number;
}
export const MAX_TEXT_BATCH_INPUTS = 128;
export const MAX_TEXT_BATCH_UNITS = 64 * 1024;
const MAX_WAITING_OPERATIONS = 256;
const MAX_WAITING_UNITS = 4 * 1024 * 1024;
/** Waiting input shares a final render, while each input retains its undo step. */
export class EditorQueue {
  private entries: QueuedOperation[] = [];
  private units = 0;
  private sizes = new WeakMap<QueuedOperation, number>();
  get length(): number { return this.entries.length; }
  get nextIsOrdinaryText(): boolean { return this.entries[0]?.operation.type === 'text' && this.entries[0].operation.inputBoundary === undefined; }
  private charge(entry: QueuedOperation, size: number): void { this.units += size; this.sizes.set(entry, (this.sizes.get(entry) ?? 0) + size); }
  push(operation: Record<string, unknown>, ticket: number, edited: boolean, epoch: number): boolean {
    const size = 2 * JSON.stringify(operation).length + 64;
    if (this.units + size > MAX_WAITING_UNITS) return false;
    let last = this.entries.at(-1);
    // A new edit reads the live viewport itself; a waiting plain repaint is redundant.
    if (last?.operation.type === 'refresh' && last.operation.followCursorGeneration === undefined
        && ['text', 'key', 'command', 'snapshot'].includes(String(operation.type))) {
      last.operation = operation; last.tickets.push(ticket); last.edited ||= edited; last.epoch = epoch;
      this.charge(last, size); return true;
    }
    const inputs = last?.operation.inputs as string[] | undefined;
    const text = operation.type === 'text' && last?.operation.type === 'text'
      && operation.inputBoundary === undefined && last.operation.inputBoundary === undefined
      && operation.followCursorGeneration === last.operation.followCursorGeneration
      && (inputs?.length ?? 1) < MAX_TEXT_BATCH_INPUTS
      && String(last.operation.text).length + String(operation.text).length <= MAX_TEXT_BATCH_UNITS;
    const refresh = operation.type === 'refresh' && last?.operation.type === 'refresh';
    const move = operation.type === 'mouse' && operation.phase === 'move' && last?.operation.type === 'mouse'
      && last.operation.phase === 'move' && last.operation.shift === operation.shift && last.operation.buttons === operation.buttons;
    if (last && (text || refresh || move)) {
      last.operation = text ? { ...operation, text: String(last.operation.text) + String(operation.text), inputs: [...(inputs ?? [String(last.operation.text)]), String(operation.text)] }
        : refresh && operation.followCursorGeneration === undefined && last.operation.followCursorGeneration !== undefined
          ? { ...operation, followCursorGeneration: last.operation.followCursorGeneration } : operation;
      last.tickets.push(ticket); last.edited ||= edited; last.epoch = epoch;
    } else {
      if (this.entries.length >= MAX_WAITING_OPERATIONS) return false;
      last = { operation, tickets: [ticket], edited, epoch };
      this.entries.push(last);
    }
    this.charge(last!, size);
    return true;
  }
  shift(): QueuedOperation | undefined {
    const entry = this.entries.shift();
    if (entry) { this.units -= this.sizes.get(entry) ?? 0; this.sizes.delete(entry); }
    return entry;
  }
}
