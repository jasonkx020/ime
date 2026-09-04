//! Desktop CLI demo shell for yc-core (M3/M3.5).
//! REPL over stdin/stdout, calling the real C ABI via yc-ffi.

use std::ffi::CString;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use yc_ffi::{
    parse_arena, yc_cold_set_callback, yc_cold_submit, yc_core_init, yc_core_shutdown,
    yc_core_sync_lang_packs, yc_hot_arena_ptr, yc_hot_arena_size, yc_hot_submit,
    yc_hw_push_stroke, yc_session_begin_with_input, yc_session_stop, ArenaCommand,
};
use yc_handwriting::templates;
use yc_types::{
    ColdKind, HotActionType, InputScheme, KeyboardLayout, WritingMode, YC_OK, YcHotAction,
    YcStrokePoint, CLASS_NUMBER, VARIATION_PASSWORD,
};

struct App {
    editor_id: u64,
    client_seq: u64,
    hw_stroke_id: u64,
    continuous_hw: bool,
    repo_root: PathBuf,
}

impl App {
    fn new(repo_root: PathBuf) -> Self {
        Self {
            editor_id: 0,
            client_seq: 0,
            hw_stroke_id: 1,
            continuous_hw: false,
            repo_root,
        }
    }

    fn hash_pack_id(id: &str) -> u32 {
        id.bytes()
            .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
    }

    fn fixture(&self, rel: &str) -> PathBuf {
        self.repo_root.join(rel)
    }

    fn submit(&mut self, action_type: HotActionType, key_code: u32, candidate_id: u32) -> i32 {
        self.client_seq += 1;
        let action = YcHotAction {
            editor_id: self.editor_id,
            client_seq: self.client_seq,
            action_type: action_type as u32,
            key_code,
            candidate_id,
            flags: 0,
            reserved: [0; 8],
        };
        yc_hot_submit(&action)
    }

    fn cold_submit_path(&self, kind: ColdKind, path: &str) -> i32 {
        yc_cold_submit(
            self.editor_id,
            kind.raw(),
            path.as_ptr(),
            path.len(),
        )
    }

    fn cold_submit_id(&self, kind: ColdKind, id: &str) -> i32 {
        yc_cold_submit(self.editor_id, kind.raw(), id.as_ptr(), id.len())
    }

    fn push_template(&mut self, text: &str) -> i32 {
        let strokes = match templates::template_strokes(text) {
            Some(s) => s,
            None => return -1,
        };
        let mode = if self.continuous_hw {
            WritingMode::Continuous
        } else {
            WritingMode::SingleChar
        };
        let mut last_rc = YC_OK;
        for stroke in strokes {
            let points: Vec<YcStrokePoint> = stroke
                .points
                .iter()
                .map(|p| YcStrokePoint {
                    x: p.x,
                    y: p.y,
                    t: p.t,
                    pressure: p.pressure,
                })
                .collect();
            last_rc = yc_hw_push_stroke(
                self.editor_id,
                points.as_ptr(),
                points.len() as u32,
                self.hw_stroke_id,
                320,
                240,
                mode.raw(),
            );
            if last_rc != YC_OK {
                return last_rc;
            }
        }
        last_rc
    }

    fn read_arena(&self) -> Option<yc_ffi::ArenaSnapshot> {
        let ptr = yc_hot_arena_ptr();
        if ptr.is_null() {
            return None;
        }
        let size = yc_hot_arena_size();
        let slice = unsafe { std::slice::from_raw_parts(ptr, size) };
        parse_arena(slice)
    }

    fn print_state(&self, stdout: &mut impl Write) {
        if let Some(snap) = self.read_arena() {
            if snap.status_flags & 1 != 0 {
                let _ = writeln!(stdout, "[连写] 等待云确认 — 输入 /confirm_cloud 或 /dismiss_cloud");
            }
            if !snap.composing.is_empty() {
                let _ = writeln!(stdout, "组字: {}", snap.composing);
            }
            if !snap.candidates.is_empty() {
                let line: Vec<String> = snap
                    .candidates
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{}.{}", i + 1, c.text))
                    .collect();
                let _ = writeln!(stdout, "候选: {}", line.join(" "));
            } else if !snap.composing.is_empty() {
                let _ = writeln!(
                    stdout,
                    "(无候选 — 请先 /install_lang + /enable_lang zh-pack-v1，再 /pinyin)"
                );
            }
            for cmd in &snap.commands {
                match cmd {
                    ArenaCommand::Commit { text } => {
                        let _ = writeln!(stdout, ">> 上屏: {}", text);
                    }
                    ArenaCommand::ApplyTheme { skin_id } => {
                        let _ = writeln!(stdout, ">> 换肤: {}", skin_id);
                    }
                    ArenaCommand::ReloadKeyboard { layout, layout_id } => {
                        let _ = writeln!(stdout, ">> 重载键盘 layout={} id={}", layout, layout_id);
                    }
                    _ => {}
                }
            }
        }
    }

    fn begin_field(&mut self, field: &str) -> u32 {
        match field {
            "password" => VARIATION_PASSWORD,
            "number" => CLASS_NUMBER,
            _ => 0,
        }
    }

    fn restart_session(&mut self, input_type: u32) {
        if self.editor_id != 0 {
            yc_session_stop(self.editor_id, 0);
        }
        self.editor_id = yc_session_begin_with_input(1, input_type);
        self.client_seq = 0;
        self.hw_stroke_id = 1;
    }
}

fn print_help(stdout: &mut impl Write) {
    let _ = writeln!(stdout, "命令:");
    let _ = writeln!(stdout, "  /pinyin | /zh        启用并切换 zh-pack-v1 中文拼音");
    let _ = writeln!(stdout, "  /clear              清空组字区");
    let _ = writeln!(stdout, "  <拼音>              逐键输入 (KeyPress；新行自动清空组字)");
    let _ = writeln!(stdout, "  /<n>                选候选 (1-based)");
    let _ = writeln!(stdout, "  /layout <name>      切换布局: pinyin26|qwerty|numeric|symbol|handwriting");
    let _ = writeln!(stdout, "  /scheme <name>      切换方案: pinyin|qwerty|handwriting");
    let _ = writeln!(stdout, "  /ascii              切换 ASCII 直出模式");
    let _ = writeln!(stdout, "  /handwriting        打开手写板");
    let _ = writeln!(stdout, "  /keyboard           返回键盘");
    let _ = writeln!(stdout, "  /hw demo <字>       模拟抬笔提交模板笔迹 (你|好|我|一|人)");
    let _ = writeln!(stdout, "  /hw continuous      连写模式开关");
    let _ = writeln!(stdout, "  /recognize          识别当前笔迹");
    let _ = writeln!(stdout, "  /confirm_cloud      确认云识别候选");
    let _ = writeln!(stdout, "  /dismiss_cloud      取消云识别");
    let _ = writeln!(stdout, "  /skin list          列出内置皮肤");
    let _ = writeln!(stdout, "  /skin apply <path>  冷路径换肤 (.imeskin)");
    let _ = writeln!(stdout, "  /install_lang <path>  安装语言包");
    let _ = writeln!(stdout, "  /enable_lang <id>     启用语言包");
    let _ = writeln!(stdout, "  /disable_lang <id>    禁用语言包");
    let _ = writeln!(stdout, "  /switch_lang <id>     切换输入语言");
    let _ = writeln!(stdout, "  /list_langs           列出已安装语言包");
    let _ = writeln!(stdout, "  /catalog [url]        拉取远程 Catalog（默认 yc-admin）");
    let _ = writeln!(stdout, "  /ai greeting|thanks|apology  本地 AI 模板建议");
    let _ = writeln!(stdout, "  /ai polish <text>     AI 润色（stub/本地）");
    let _ = writeln!(stdout, "  /field <name>       切换输入框: default|password|number");
    let _ = writeln!(stdout, "  /help               显示帮助");
    let _ = writeln!(stdout, "  /quit               退出");
}

fn parse_layout(name: &str) -> Option<KeyboardLayout> {
    match name.to_ascii_lowercase().as_str() {
        "pinyin26" | "pinyin" => Some(KeyboardLayout::Pinyin26),
        "qwerty" => Some(KeyboardLayout::Qwerty),
        "numeric" => Some(KeyboardLayout::Numeric),
        "symbol" => Some(KeyboardLayout::Symbol),
        "handwriting" | "hw" => Some(KeyboardLayout::HandwritingPad),
        _ => None,
    }
}

fn parse_scheme(name: &str) -> Option<InputScheme> {
    match name.to_ascii_lowercase().as_str() {
        "pinyin" | "pinyinfull" => Some(InputScheme::PinyinFull),
        "qwerty" => Some(InputScheme::Qwerty),
        "handwriting" | "hw" => Some(InputScheme::Handwriting),
        _ => None,
    }
}

use std::sync::OnceLock;

static COLD_DONE: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

extern "C" fn on_cold_callback(
    task_id: i32,
    _editor_id: u64,
    payload: *const u8,
    len: usize,
    err: i32,
) {
    if err != 0 || payload.is_null() || len == 0 {
        return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(payload, len) };
    if let Some(slot) = COLD_DONE.get() {
        if let Ok(tokens) = yc_theme::ThemeTokens::from_json_bytes(bytes) {
            let mut guard = slot.lock().unwrap();
            *guard = Some(format!("skin:{} task={}", tokens.skin_id, task_id));
        } else if let Ok(s) = std::str::from_utf8(bytes) {
            let mut guard = slot.lock().unwrap();
            *guard = Some(format!("cold:{} task={}", s, task_id));
        }
    }
}

fn find_repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..6 {
        if dir.join("fixtures").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    PathBuf::from(".")
}

fn main() {
    let repo_root = find_repo_root();
    let data_dir = std::env::temp_dir().join("yc_cli_data");
    let _ = std::fs::create_dir_all(&data_dir);
    let dir = CString::new(data_dir.to_string_lossy().into_owned()).expect("data_dir");
    if yc_core_init(dir.as_ptr()) != YC_OK {
        eprintln!("yc_core_init failed");
        std::process::exit(1);
    }

    let cold_done: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let _ = COLD_DONE.set(cold_done.clone());
    yc_cold_set_callback(Some(on_cold_callback));

    let mut app = App::new(repo_root);
    app.restart_session(0);

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let _ = writeln!(stdout, "yc-cli M3/M3.5 demo — 输入 /help 查看命令");
    let _ = writeln!(
        stdout,
        "提示: /install_lang {} → /enable_lang zh-pack-v1 → /pinyin",
        app.fixture("fixtures/dist/zh-pack-v1.imepack").display()
    );
    let _ = write!(stdout, "yc-cli> ");
    let _ = stdout.flush();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(_) => break,
        };

        if line.is_empty() {
            if let Some(msg) = cold_done.lock().unwrap().take() {
                let _ = writeln!(stdout, "[冷路径完成] {}", msg);
            }
            let _ = write!(stdout, "yc-cli> ");
            let _ = stdout.flush();
            continue;
        }

        if line == "/quit" || line == "/exit" {
            break;
        }

        if line == "/help" {
            print_help(&mut stdout);
        } else if line == "/handwriting" {
            let rc = app.submit(HotActionType::OpenHandwriting, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "打开手写板失败: {}", rc);
            } else {
                let _ = writeln!(stdout, "已打开手写板");
            }
        } else if line == "/keyboard" {
            let rc = app.submit(HotActionType::DismissHandwriting, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "返回键盘失败: {}", rc);
            } else {
                let _ = writeln!(stdout, "已返回键盘");
            }
        } else if line == "/recognize" {
            let rc = app.submit(HotActionType::RecognizeHandwriting, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "识别失败: {}", rc);
            }
        } else if line == "/confirm_cloud" {
            let rc = app.submit(HotActionType::ConfirmCloudHandwriting, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "云确认失败: {}", rc);
            }
        } else if line == "/dismiss_cloud" {
            let rc = app.submit(HotActionType::DismissCloudHandwriting, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "取消云识别失败: {}", rc);
            }
        } else if line == "/hw continuous" || line == "/hwcontinuous" {
            app.continuous_hw = !app.continuous_hw;
            let _ = writeln!(
                stdout,
                "连写模式: {}",
                if app.continuous_hw { "开" } else { "关" }
            );
        } else if line == "/hwclear" {
            let rc = app.submit(HotActionType::ClearHandwriting, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "清空失败: {}", rc);
            }
        } else if line == "/hwundo" {
            let rc = app.submit(HotActionType::UndoHandwriting, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "撤销失败: {}", rc);
            }
        } else if let Some(rest) = line.strip_prefix("/hw demo ") {
            let ch = rest.trim();
            let rc = app.push_template(ch);
            if rc != YC_OK {
                let _ = writeln!(stdout, "提交笔迹失败: {} (支持: 你|好|我|一|人)", rc);
            } else {
                let _ = writeln!(stdout, "已提交「{}」模板笔迹，输入 /recognize 识别", ch);
            }
        } else if line == "/skin list" {
            let _ = writeln!(stdout, "  samsung-light (内置默认)");
            let skin = app.fixture("fixtures/dist/samsung-light.imeskin");
            if skin.exists() {
                let _ = writeln!(stdout, "  {} (fixture)", skin.display());
            }
        } else if let Some(rest) = line.strip_prefix("/skin apply ") {
            let path = if rest.contains('/') || rest.contains('\\') {
                PathBuf::from(rest)
            } else {
                app.fixture(&format!("fixtures/dist/{}.imeskin", rest.trim()))
            };
            if !path.exists() {
                let _ = writeln!(stdout, "皮肤文件不存在: {}", path.display());
            } else {
                let p = path.to_string_lossy();
                let rc = app.cold_submit_path(ColdKind::Skin, &p);
                thread::sleep(Duration::from_millis(120));
                let _ = writeln!(stdout, "冷路径换肤提交: rc={}", rc);
            }
        } else if let Some(rest) = line.strip_prefix("/install_lang ") {
            let path = PathBuf::from(rest.trim());
            let p = path.to_string_lossy();
            let rc = app.cold_submit_path(ColdKind::LangPackInstall, &p);
            thread::sleep(Duration::from_millis(120));
            let _ = writeln!(stdout, "安装提交: rc={}", rc);
        } else if let Some(id) = line.strip_prefix("/enable_lang ") {
            let id = id.trim();
            let rc = app.cold_submit_id(ColdKind::LangPackEnable, id);
            thread::sleep(Duration::from_millis(120));
            yc_core_sync_lang_packs();
            let _ = writeln!(stdout, "启用 {}: rc={} (已 reconcile)", id, rc);
        } else if let Some(id) = line.strip_prefix("/disable_lang ") {
            let id = id.trim();
            let rc = app.cold_submit_id(ColdKind::LangPackDisable, id);
            thread::sleep(Duration::from_millis(120));
            yc_core_sync_lang_packs();
            let _ = writeln!(stdout, "禁用 {}: rc={} (已 reconcile)", id, rc);
        } else if let Some(id) = line.strip_prefix("/switch_lang ") {
            let id = id.trim();
            let hash = App::hash_pack_id(id);
            let rc = app.submit(HotActionType::SwitchLang, hash, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "切换语言失败: {}", rc);
            } else {
                let _ = writeln!(stdout, "已切换至 {}", id);
            }
        } else if line == "/list_langs" {
            let pack = app.fixture("fixtures/dist/vi-v1.imepack");
            let _ = writeln!(stdout, "fixture: {}", pack.display());
            let _ = writeln!(stdout, "使用 /install_lang /enable_lang /switch_lang vi-v1 验收");
        } else if line == "/catalog" || line.starts_with("/catalog ") {
            let url = line
                .strip_prefix("/catalog")
                .unwrap_or("")
                .trim();
            let url = if url.is_empty() {
                "http://127.0.0.1:8080/api/v1/catalog"
            } else {
                url
            };
            let rc = app.cold_submit_id(ColdKind::LangPackCatalog, url);
            thread::sleep(Duration::from_millis(200));
            let _ = writeln!(
                stdout,
                "fetch catalog {} rc={} (需 yc-admin 已启动；结果经冷路径回调)",
                url, rc
            );
        } else if let Some(rest) = line.strip_prefix("/layout ") {
            if let Some(layout) = parse_layout(rest.trim()) {
                let rc = app.submit(HotActionType::SwitchLayout, layout.raw(), 0);
                if rc != YC_OK {
                    let _ = writeln!(stdout, "切换布局失败: {}", rc);
                } else {
                    let _ = writeln!(stdout, "已切换布局: {}", rest.trim());
                }
            } else {
                let _ = writeln!(stdout, "未知布局: {}", rest);
            }
        } else if let Some(rest) = line.strip_prefix("/scheme ") {
            if let Some(scheme) = parse_scheme(rest.trim()) {
                let rc = app.submit(HotActionType::SwitchScheme, scheme.raw(), 0);
                if rc != YC_OK {
                    let _ = writeln!(stdout, "切换方案失败: {}", rc);
                } else {
                    let _ = writeln!(stdout, "已切换方案: {}", rest.trim());
                }
            } else {
                let _ = writeln!(stdout, "未知方案: {}", rest);
            }
        } else if line == "/ascii" {
            let rc = app.submit(HotActionType::ToggleAscii, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "切换 ASCII 失败: {}", rc);
            } else {
                let _ = writeln!(stdout, "已切换 ASCII 模式");
            }
        } else if let Some(rest) = line.strip_prefix("/field ") {
            let input_type = app.begin_field(rest.trim());
            app.restart_session(input_type);
            let _ = writeln!(
                stdout,
                "已切换输入框: {} (input_type=0x{:x})",
                rest.trim(),
                input_type
            );
        } else if let Some(rest) = line.strip_prefix("/ai ") {
            let rest = rest.trim();
            let svc = yc_ai::AiAssistService::new();
            let privacy = yc_types::PrivacyLevel::Normal;
            if let Some(text) = rest.strip_prefix("polish ") {
                let req = yc_types::TaskReq {
                    editor_id: app.editor_id,
                    mode: yc_types::AiMode::Polish.raw(),
                    scene_id: "polish".into(),
                    peer_message: String::new(),
                    background_note: String::new(),
                    selection_text: text.trim().into(),
                    user_intent: String::new(),
                };
                let preview = svc.preview_payload(&req);
                let _ = writeln!(stdout, "preview: {}", preview.summary);
                match svc.polish(privacy, &req) {
                    Ok(out) => {
                        for (i, v) in out.variants.iter().enumerate() {
                            let _ = writeln!(stdout, "  [{}] ({}) {}", i + 1, v.tone, v.text);
                        }
                    }
                    Err(e) => {
                        let _ = writeln!(stdout, "AI 润色失败: {:?}", e);
                    }
                }
            } else {
                let scene = if rest.is_empty() { "greeting" } else { rest };
                let req = yc_types::TaskReq {
                    editor_id: app.editor_id,
                    mode: yc_types::AiMode::Compose.raw(),
                    scene_id: scene.into(),
                    peer_message: String::new(),
                    background_note: String::new(),
                    selection_text: String::new(),
                    user_intent: String::new(),
                };
                match svc.suggest(privacy, &req) {
                    Ok(out) => {
                        let _ = writeln!(
                            stdout,
                            "AI suggest scene={} local={}",
                            scene, out.local
                        );
                        for (i, v) in out.variants.iter().enumerate() {
                            let _ = writeln!(stdout, "  [{}] ({}) {}", i + 1, v.tone, v.text);
                        }
                    }
                    Err(e) => {
                        let _ = writeln!(stdout, "AI 失败: {:?}", e);
                    }
                }
            }
        } else if line == "/pinyin" || line == "/zh" {
            let pack = app.fixture("fixtures/dist/zh-pack-v1.imepack");
            if pack.exists() {
                let p = pack.to_string_lossy();
                let _ = app.cold_submit_path(ColdKind::LangPackInstall, &p);
                thread::sleep(Duration::from_millis(80));
                let _ = app.cold_submit_id(ColdKind::LangPackEnable, "zh-pack-v1");
                thread::sleep(Duration::from_millis(80));
                yc_core_sync_lang_packs();
            }
            if let Some(layout) = parse_layout("pinyin26") {
                let rc = app.submit(HotActionType::SwitchLayout, layout.raw(), 0);
                let _ = writeln!(
                    stdout,
                    "已切换中文拼音 zh-pack-v1 (rc={})，输入 nihao 后 /1",
                    rc
                );
            }
        } else if line == "/clear" {
            let rc = app.submit(HotActionType::Init, 0, 0);
            if rc != YC_OK {
                let _ = writeln!(stdout, "清空组字失败: {}", rc);
            } else {
                let _ = writeln!(stdout, "已清空组字");
            }
        } else if let Some(rest) = line.strip_prefix('/') {
            if let Ok(idx) = rest.parse::<u32>() {
                if idx == 0 {
                    let _ = writeln!(stdout, "候选序号从 1 开始");
                } else {
                    let rc = app.submit(HotActionType::SelectCandidate, 0, idx - 1);
                    if rc != YC_OK {
                        let _ = writeln!(stdout, "选词失败: {}", rc);
                    }
                }
            } else {
                let _ = writeln!(stdout, "未知命令: {}", line);
            }
        } else {
            // REPL 每行视为独立试输入，避免组字跨行累积（如 go + nihao → gonihao）
            let _ = app.submit(HotActionType::Init, 0, 0);
            for ch in line.chars() {
                if ch.is_ascii_alphabetic() {
                    let rc = app.submit(HotActionType::KeyPress, ch as u32, 0);
                    if rc != YC_OK {
                        let _ = writeln!(stdout, "按键失败 '{}': {}", ch, rc);
                    }
                }
            }
        }

        if let Some(msg) = cold_done.lock().unwrap().take() {
            let _ = writeln!(stdout, "[冷路径完成] {}", msg);
        }
        app.print_state(&mut stdout);
        let _ = write!(stdout, "yc-cli> ");
        let _ = stdout.flush();
    }

    if app.editor_id != 0 {
        yc_session_stop(app.editor_id, 0);
    }
    yc_core_shutdown();
}
