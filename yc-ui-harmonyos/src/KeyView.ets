import { ThemeTokens, KeyboardSnapshot, CandidateItem } from './ThemeTokens';

@Component
export struct SamsungCandBar {
  @Prop snapshot: KeyboardSnapshot = { editorId: 0, seq: 0, composing: '', candidates: [] };
  @Prop tokens: ThemeTokens = new ThemeTokens();
  onCandidate: (item: CandidateItem) => void = () => {};

  build() {
    Row() {
      if (this.snapshot.composing.length > 0) {
        Text(this.snapshot.composing)
          .fontSize(13)
          .fontColor(this.tokens.composingText)
          .margin({ right: 12 })
      }
      ForEach(this.snapshot.candidates, (c: CandidateItem) => {
        Button(c.text)
          .fontSize(15)
          .fontColor(this.tokens.candText)
          .backgroundColor(Color.White)
          .border({ width: 1, color: this.tokens.keyAccent, radius: 16 })
          .padding({ left: 10, right: 10, top: 6, bottom: 6 })
          .margin({ right: 6 })
          .onClick(() => this.onCandidate(c))
      })
    }
    .width('100%')
    .height(52)
    .padding({ left: 10, right: 10 })
    .backgroundColor(this.tokens.keyboardBg)
  }
}

@Component
export struct SamsungKeyView {
  @Prop tokens: ThemeTokens = new ThemeTokens();
  onKey: (label: string, code: number) => void = () => {};

  private rows: string[][] = [
    ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
    ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', '⌫'],
    ['z', 'x', 'c', 'v', 'b', 'n', 'm'],
    ['!#1', '🌐', ',', '空格', '。', '搜索'],
  ];

  build() {
    Column({ space: 6 }) {
      ForEach(this.rows, (row: string[]) => {
        Row({ space: 3 }) {
          ForEach(row, (label: string) => {
            Button(label)
              .layoutWeight(label === '空格' ? 3.6 : 1)
              .height(44)
              .fontSize(16)
              .fontColor(label === '搜索' ? Color.White : this.tokens.candText)
              .backgroundColor(this.keyBg(label))
              .borderRadius(this.tokens.keyRadius)
              .onClick(() => this.onKey(label, this.keyCode(label)))
          })
        }
        .width('100%')
      })
    }
    .padding(12)
    .backgroundColor(this.tokens.keyboardBg)
  }

  private keyBg(label: string): string {
    if (label === '搜索') return this.tokens.keyAccent;
    if (label === '!#1' || label === '🌐' || label === '⌫') return this.tokens.keyUtility;
    return this.tokens.keyNormal;
  }

  private keyCode(label: string): number {
    if (label === '⌫') return 8;
    if (label === '空格') return 32;
    if (label.length === 1) return label.charCodeAt(0);
    return 0;
  }
}
