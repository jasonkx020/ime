// 候选栏占位。参见 docs/KEYBOARD_UI_DESIGN.md §3.1。

export interface CandBar {
  render(snapshot: object): void
  applyTheme(tokens: object): void
}
