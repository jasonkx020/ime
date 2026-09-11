import ycNative from 'libyc_native.so';

export interface ArenaCandidate {
  id: number;
  text: string;
}

export type ArenaCommand =
  | { kind: 'commit'; text: string }
  | { kind: 'setComposing'; text: string }
  | { kind: 'finishComposing' }
  | { kind: 'deleteSurrounding'; before: number; after: number }
  | { kind: 'reloadKeyboard'; layout: number; layoutId: string };

export interface ArenaSnapshot {
  editorId: number;
  seq: number;
  statusFlags: number;
  composing: string;
  candidates: ArenaCandidate[];
  commands: ArenaCommand[];
}

const HEADER_SIZE = 32;
const COMPOSING_LEN = 64;
const CAND_SLOT_SIZE = 80;
const CMD_SLOT_SIZE = 80;
const MAX_CANDIDATES = 9;
const MAX_COMMANDS = 4;

export function parseArena(bytes: number[]): ArenaSnapshot | null {
  if (bytes.length < HEADER_SIZE) {
    return null;
  }
  const view = new DataView(new Uint8Array(bytes).buffer);
  const editorId = Number(view.getBigUint64(0, true));
  const seq = Number(view.getBigUint64(8, true));
  const statusFlags = view.getUint32(16, true);
  const composingLen = Math.min(view.getUint32(20, true), COMPOSING_LEN);
  const candCount = Math.min(view.getUint32(24, true), MAX_CANDIDATES);
  const cmdCount = Math.min(view.getUint32(28, true), MAX_COMMANDS);

  const composing = utf8Decode(bytes.slice(HEADER_SIZE, HEADER_SIZE + composingLen));
  const candidates: ArenaCandidate[] = [];
  const slotsOff = HEADER_SIZE + COMPOSING_LEN;
  for (let i = 0; i < candCount; i++) {
    const off = slotsOff + i * CAND_SLOT_SIZE;
    const id = view.getUint32(off, true);
    const textLen = Math.min(view.getUint32(off + 8, true), 64);
    const text = utf8Decode(bytes.slice(off + 16, off + 16 + textLen));
    candidates.push({ id, text });
  }

  const commands: ArenaCommand[] = [];
  const cmdsOff = slotsOff + MAX_CANDIDATES * CAND_SLOT_SIZE;
  for (let i = 0; i < cmdCount; i++) {
    const off = cmdsOff + i * CMD_SLOT_SIZE;
    const cmdType = view.getUint32(off, true);
    const param0 = view.getUint32(off + 4, true);
    const param1 = view.getUint32(off + 8, true);
    const textLen = Math.min(view.getUint32(off + 12, true), 64);
    const text = utf8Decode(bytes.slice(off + 16, off + 16 + textLen));
    switch (cmdType) {
      case 0: commands.push({ kind: 'commit', text }); break;
      case 1: commands.push({ kind: 'setComposing', text }); break;
      case 2: commands.push({ kind: 'finishComposing' }); break;
      case 3: commands.push({ kind: 'deleteSurrounding', before: param0, after: param1 }); break;
      case 4: commands.push({ kind: 'reloadKeyboard', layout: param0, layoutId: text }); break;
    }
  }

  return { editorId, seq, statusFlags, composing, candidates, commands };
}

function utf8Decode(bytes: number[]): string {
  const arr = new Uint8Array(bytes);
  let out = '';
  for (let i = 0; i < arr.length; i++) {
    out += String.fromCharCode(arr[i]);
  }
  return out;
}

export class YcHotPathClient {
  editorId: number = 0;
  private clientSeq: number = 0;
  lastSeq: number = 0;

  initCore(dataDir: string): number {
    return ycNative.ycCoreInit(dataDir) as number;
  }

  shutdown(): void {
    ycNative.ycCoreShutdown();
  }

  beginSession(fieldId: number, inputType: number = 0): number {
    this.editorId = ycNative.ycSessionBeginWithInput(fieldId, inputType) as number;
    this.clientSeq = 0;
    this.lastSeq = 0;
    if (this.editorId !== 0) {
      this.submit(0);
    }
    return this.editorId;
  }

  stopSession(reason: number = 0): void {
    if (this.editorId !== 0) {
      ycNative.ycSessionStop(this.editorId, reason);
      this.editorId = 0;
    }
  }

  submit(actionType: number, keyCode: number = 0, candidateId: number = 0): number {
    if (this.editorId === 0) {
      return -1;
    }
    this.clientSeq += 1;
    const action: number[] = new Array(40).fill(0);
    const view = new DataView(new Uint8Array(action).buffer);
    view.setBigUint64(0, BigInt(this.editorId), true);
    view.setBigUint64(8, BigInt(this.clientSeq), true);
    view.setUint32(16, actionType, true);
    view.setUint32(20, keyCode, true);
    view.setUint32(24, candidateId, true);
    return ycNative.ycHotSubmit(action) as number;
  }

  readArena(): ArenaSnapshot | null {
    const bytes = ycNative.ycHotArenaBytes() as number[];
    return parseArena(bytes);
  }
}

export default ycNative;
