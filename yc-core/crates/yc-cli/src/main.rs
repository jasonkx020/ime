//! Desktop CLI demo shell for yc-core (M2/M2.5).
//! REPL over stdin/stdout, calling the real C ABI via yc-ffi.

use std::ffi::CString;
use std::io::{self, BufRead, Write};

use yc_ffi::{
    parse_arena, yc_core_init, yc_core_shutdown, yc_hot_arena_ptr, yc_hot_arena_size,
    yc_hot_submit, yc_hw_push_stroke, yc_session_begin_with_input, yc_session_stop,
};
use yc_handwriting::templates;
use yc_types::{
    HotActionType, KeyboardLayout, InputScheme, YC_OK, YcHotAction, YcStrokePoint, CLASS_NUMBER,
    VARIATION_PASSWORD, WritingMode,
};

struct App {
    editor_id: u64,
    client_seq: u64,
    hw_stroke_id: u64,
}

impl App {
    fn new() -> Self {
        Self {
            editor_id: 0,
            client_seq: 0,
            hw_stroke_id: 1,
        }
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

    fn push_template(&mut self, text: &str) -> i32 {
        let strokes = match templates::template_strokes(text) {
            Some(s) => s,
            None => return -1,
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
                WritingMode::SingleChar.raw(),
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
    let _ = writeln!(stdout, "  <拼音>              逐键输入 (KeyPress)");
    let _ = writeln!(stdout, "  /<n>                选候选 (1-based)");
    let _ = writeln!(stdout, "  /layout <name>      切换布局: pinyin26|qwerty|numeric|symbol|handwriting");
    let _ = writeln!(stdout, "  /scheme <name>      切换方案: pinyin|qwerty|handwriting");
    let _ = writeln!(stdout, "  /ascii              切换 ASCII 直出模式");
    let _ = writeln!(stdout, "  /handwriting        打开手写板");
    let _ = writeln!(stdout, "  /keyboard           返回键盘");
    let _ = writeln!(stdout, "  /hw demo <字>       模拟抬笔提交模板笔迹 (你|好|我|一|人)");
    let _ = writeln!(stdout, "  /recognize          识别当前笔迹");
    let _ = writeln!(stdout, "  /hwclear            清空书写区");
    let _ = writeln!(stdout, "  /hwundo             撤销上一笔");
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

fn main() {
    let dir = CString::new(".").expect("cwd");
    if yc_core_init(dir.as_ptr()) != YC_OK {
        eprintln!("yc_core_init failed");
        std::process::exit(1);
    }

    let mut app = App::new();
    app.restart_session(0);

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let _ = writeln!(stdout, "yc-cli M2.5 demo — 输入 /help 查看命令");
    let _ = write!(stdout, "yc-cli> ");
    let _ = stdout.flush();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(_) => break,
        };

        if line.is_empty() {
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
            let _ = writeln!(stdout, "已切换输入框: {} (input_type=0x{:x})", rest.trim(), input_type);
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
            for ch in line.chars() {
                if ch.is_ascii_alphabetic() {
                    let rc = app.submit(HotActionType::KeyPress, ch as u32, 0);
                    if rc != YC_OK {
                        let _ = writeln!(stdout, "按键失败 '{}': {}", ch, rc);
                    }
                }
            }
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
