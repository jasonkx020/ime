# 娱乐与输入一体（产品落地）

> 原则：**输入是主路径，娱乐是可卸载旁路。** P0 按键/候选路径零广告。

## 分层

| 进程 | 职责 |
|------|------|
| IME（轻） | 皮肤快切、系统 Emoji、话术卡面板、语音 ASR；只应用 `ThemeTokens` |
| 主 App（重） | `MainActivity` 底栏：首页 / 发现 / 设置；语言在「设置 → 语言与布局」 |

## P0 已落地

- 工具栏 `FeatureBar`：设置 / 皮肤 / 表情 / 话术 / 手写
- `SkinPicker` + `SkinRegistry`：内置 `samsung-light` / `samsung-dark` / 跟随系统；换肤即时 `applyTheme`
- 主 App「发现」列出已装皮肤并切换（下次打开键盘生效；键盘内快切立即生效）

## P1 已落地

- 本地 Catalog：`assets/catalog/discover_catalog.json`（`kind`: langpack \| skin \| content）
- `yc-admin` `CatalogEntry.Kind` 字段；发布语言包时默认 `kind=langpack`
- 行业 ContentPack 样例：
  - `assets/content/industry-ecommerce-v1`（电商客服词库 + 话术卡）
  - `assets/content/industry-gaming-v1`（游戏黑话 + 话术卡）
- 发现页频道：皮肤 / 行业包 / 活动 / 设置

## P2 已落地（MVP）

- 节日活动：Catalog `campaigns` + 发现页「参与」一键套皮肤与话术包
- 话术卡：键盘「话术」→ `PhraseDeckPanel`（本地 JSON，不调 LLM）
- 账号/付费：设置页预留说明（SKU 架构位，首期免费）
- 真 ASR：`SpeechHost` 优先 `SpeechRecognizer`，不可用时 stub 回退

## 内容包约定（`.imecontent` 源目录）

```text
content.toml     # id / version / kind / langs / privacy
lexicon/*.tsv    # 行业词（可选）
phrases/*.json   # 话术卡（可选）
```

热路径不加载贴纸大图；行业词后续可合入 lexicon 增量或 `UserWordStore.apply_boost`。

## 体验红线

1. P0 零广告  
2. 换肤 ≤100ms（token 同步，大图异步）  
3. 发现页崩溃不影响键盘  
4. ForbiddenCloud / 密码框不展示娱乐入口、不学词  
5. 皮肤 / 贴纸 / 行业包均可删；默认皮肤与核心语言包常驻  
