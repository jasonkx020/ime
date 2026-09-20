# yc-admin

Go 管理后台：语言包编辑/发布、用户画像分析、输入与选词习惯学习，并生成个性化选词 boost，供客户端 `LightIntel` / `user_words` 提升候选体验。

## 能力

| 模块 | 说明 |
|------|------|
| 语言包 | 草稿 → 上传 `.imepack` → review → publish → Catalog / CDN |
| Catalog | `GET /api/v1/catalog`；条目 `kind`: `langpack` \| `skin` \| `content`（发布语言包默认 `langpack`；skin/content 可由运营 Catalog 扩展）。**不做** LLM proxy / API Key 托管（AI 为用户 BYOK 直连） |
| 习惯上报 | `POST /api/v1/habits/events`（仅 `privacy_ok=true`） |
| 用户画像 | 语言偏好、高频码/词、平均选词位、回删率、persona tags |
| 选词优化 | 按选词频次与候选位次计算 boost，下发 personalization pack |

**与端侧 BYOK 画像的边界**：Android 可在设备上用用户配置的 LLM（`yc_llm_byok`）异步优化 `prefer_pairs` / demote / boost，经 `yc_personalization_apply` 写入本地；**API Key 不上传本服务**。admin HabitSummarizer 仍为云端规则汇总（可选 SyncWorker 拉取），与端侧 LLM 画像并行、互不托管密钥。热路径按键/选词不得等待 LLM。

## 快速启动

```powershell
cd services/yc-admin
go mod tidy
go run ./cmd/yc-admin
```

默认：

- 监听 `:8080`
- 数据目录 `./data`（SQLite + `packs/`）
- Admin Token：`dev-token`（请求头 `X-Admin-Token`）
- 管理台：http://127.0.0.1:8080/

环境变量：

| 变量 | 默认 | 说明 |
|------|------|------|
| `YC_ADMIN_ADDR` | `:8080` | 监听地址 |
| `YC_ADMIN_DATA` | `./data` | 数据目录 |
| `YC_ADMIN_TOKEN` | `dev-token` | 管理写接口鉴权 |
| `YC_ADMIN_PUBLIC_BASE` | `http://127.0.0.1:8080` | Catalog 内 CDN URL 前缀 |

## API 摘要

### 管理（需 Token）

```http
GET  /api/v1/dashboard
GET  /api/v1/langpacks
POST /api/v1/langpacks
PATCH /api/v1/langpacks/{id}
POST /api/v1/langpacks/{id}/upload   # multipart field=file
POST /api/v1/langpacks/{id}/publish
POST /api/v1/langpacks/{id}/archive
GET  /api/v1/profiles
GET  /api/v1/profiles/{device_id}
POST /api/v1/profiles/{device_id}/rebuild
POST /api/v1/personalization/rebuild-all
```

### 客户端公开

```http
GET  /api/v1/catalog
GET  /cdn/langpacks/{file}.imepack
POST /api/v1/habits/events
GET  /api/v1/personalization/{device_id}
```

### 习惯事件示例

```json
{
  "events": [
    {
      "device_id": "d-001",
      "lang": "zh",
      "pack_id": "zh-pack-v1",
      "event_type": "select",
      "query_key": "ta",
      "selected_word": "他",
      "candidate_pos": 3,
      "privacy_ok": true
    }
  ]
}
```

隐私约定：只上报 `query_key` + 选中词 + 候选位次 + `lang`/`pack_id`；**不要**上报 composing 明文、聊天上下文或敏感字段。`privacy_ok=false` 的事件会被丢弃。

### Personalization 示例

```json
{
  "device_id": "d-001",
  "lang": "en",
  "version": 1710000000,
  "boosts": [
    { "query_key": "th", "word": "thanks", "boost": 10.5, "freq": 3, "lang": "en" }
  ],
  "prefer_pairs": [
    { "prev": "thank", "next": "you", "delta": 1.5 }
  ],
  "persona_tags": ["needs_rerank", "lang_en"],
  "expiry_hours": 168
}
```

客户端可将 `boosts` 合并进本地 `user_words`（键：`lang\\tquery_key\\tword`），或在 `LightIntel.rerank` 中按 `boost` / `prefer_pairs` 提升排序。`pinyin` 字段为 `query_key` 的兼容别名。

## 与 yc-core 的边界

- **构建**仍用 `tools/ime-pack` / `yc-pack`；本服务只负责托管、发布与 Catalog。
- **热路径选词**仍在 Rust；本服务产出个性化增量，经 SyncWorker / 冷路径下发。
- 不替代 `PluginHost` 验签安装；生产环境请在 CDN 前叠加 Ed25519 签名与对象存储。

## 目录

```text
cmd/yc-admin/          入口
internal/config/       配置
internal/model/        DTO
internal/store/        SQLite
internal/service/      语言包 / 画像 / boost
internal/api/          HTTP
web/                   简易管理台
```
