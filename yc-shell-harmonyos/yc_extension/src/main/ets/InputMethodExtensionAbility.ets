import { InputMethodExtensionAbility } from '@kit.IMEKit';
import inputMethodEngine from '@ohos.inputMethodEngine';
import { YcHotPathClient } from './YcArena';
import { ThemeTokens, KeyboardSnapshot, CandidateItem } from '../../../yc-ui-harmonyos/src/ThemeTokens';
import { SamsungCandBar, SamsungKeyView } from '../../../yc-ui-harmonyos/src/KeyView';

export default class YcInputExtensionAbility extends InputMethodExtensionAbility {
  private client: YcHotPathClient = new YcHotPathClient();
  private snapshot: KeyboardSnapshot = { editorId: 0, seq: 0, composing: '', candidates: [] };
  private tokens: ThemeTokens = new ThemeTokens();
  private panel: inputMethodEngine.Panel | null = null;

  onCreate(): void {
    this.client.initCore(this.context.filesDir);
    this.client.beginSession(1, 0);
    this.createPanel();
  }

  onDestroy(): void {
    this.client.stopSession();
    this.client.shutdown();
  }

  private createPanel(): void {
  }

  private refreshUi(): void {
    const snap = this.client.readArena();
    if (!snap || snap.editorId !== this.client.editorId) {
      return;
    }
    this.snapshot = {
      editorId: snap.editorId,
      seq: snap.seq,
      composing: snap.composing,
      candidates: snap.candidates,
    };
    for (const cmd of snap.commands) {
      if (cmd.kind === 'commit') {
        console.info(`[yc-harmony] commit: ${cmd.text}`);
      }
      if (cmd.kind === 'reloadKeyboard' && cmd.layoutId) {
        console.info(`[yc-harmony] reload layout: ${cmd.layoutId}`);
      }
    }
  }

  onKey(label: string, code: number): void {
    if (code === 8) {
      this.client.submit(2);
    } else if (code > 0) {
      this.client.submit(1, code);
    }
    this.refreshUi();
  }

  onCandidate(item: CandidateItem): void {
    this.client.submit(3, 0, item.id);
    this.refreshUi();
  }
}
