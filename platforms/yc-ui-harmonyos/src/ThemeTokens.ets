// Samsung One UI 6.x theme tokens (KEYBOARD_UI_DESIGN §11.1).

export class ThemeTokens {
  keyboardBg: string = '#E8EAED';
  keyNormal: string = '#FFFFFF';
  keyUtility: string = '#DDE0E4';
  keyAccent: string = '#1A73E8';
  keyPressed: string = '#C8CCD2';
  composingText: string = '#1A73E8';
  candText: string = '#202124';
  toolbarText: string = '#5F6368';
  keyRadius: number = 12;
}

export interface CandidateItem {
  id: number;
  text: string;
}

export interface KeyboardSnapshot {
  editorId: number;
  seq: number;
  composing: string;
  candidates: CandidateItem[];
}
