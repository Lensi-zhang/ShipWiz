//! TUI 启动动画：舰船冲锋破浪，独占终端画布
//!
//! 主程序启动时调用 [`play`]，动画播完后自动交还画布；
//! 独立预览：cargo run --example splash_demo

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use std::io;
use std::time::{Duration, Instant};

// ---------- 配色（与 TUI / 桌面端保持一致） ----------
const BG: Color = Color::Rgb(5, 16, 26);
const SKY_DEEP: Color = Color::Rgb(7, 22, 34);
const STAR: Color = Color::Rgb(158, 186, 204);
const MUTED: Color = Color::Rgb(114, 143, 157);
const ACCENT: Color = Color::Rgb(67, 198, 217);
const GOLD: Color = Color::Rgb(227, 179, 65);
const GOLD_DIM: Color = Color::Rgb(146, 114, 42);
const FOAM: Color = Color::Rgb(236, 248, 252);

// ---------- 时间轴（毫秒） ----------
const T_CHARGE_START: f64 = 240.0;
const T_CHARGE_END: f64 = 1120.0;
const T_TITLE_START: f64 = 1020.0;
const T_TITLE_STEP: f64 = 80.0;
const T_TITLE_FADE: f64 = 240.0;

/// 动画总时长：播完自动收尾
pub const DURATION_MS: f64 = 4200.0;

// ---------- 舰船尺寸（船首朝右，程序化绘制块状剪影） ----------
const SHIP_LEN: i32 = 46;
const SHIP_HEIGHT: i32 = 6;

struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<(char, Color, Color)>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![(' ', BG, BG); width * height],
        }
    }

    fn put(&mut self, x: i32, y: i32, ch: char, color: Color) {
        self.put_bg(x, y, ch, color, BG);
    }

    fn put_bg(&mut self, x: i32, y: i32, ch: char, color: Color, bg: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width || y >= self.height {
            return;
        }
        self.cells[y * self.width + x] = (ch, color, bg);
    }

    fn text(&mut self, x: i32, y: i32, text: &str, color: Color) {
        for (offset, ch) in text.chars().enumerate() {
            self.put(x + offset as i32, y, ch, color);
        }
    }

    /// 只改字符与前景色，保留该格原有背景：水面粒子用，避免出现深色方块
    fn paint(&mut self, x: i32, y: i32, ch: char, color: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width || y >= self.height {
            return;
        }
        let index = y * self.width + x;
        let bg = self.cells[index].2;
        self.cells[index] = (ch, color, bg);
    }

    /// 只给已有的像素染色，不覆盖字符与背景：用于拖影，避免抹掉水纹
    fn tint(&mut self, x: i32, y: i32, color: Color, amount: f64) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width || y >= self.height {
            return;
        }
        let index = y * self.width + x;
        let (ch, fg, bg) = self.cells[index];
        if ch == ' ' {
            return;
        }
        self.cells[index] = (ch, mix(fg, color, amount), bg);
    }

    fn lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::with_capacity(self.height);
        for y in 0..self.height {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut run = String::new();
            let (_, mut fg, mut bg) = self.cells[y * self.width];
            for x in 0..self.width {
                let (ch, color, back) = self.cells[y * self.width + x];
                if color == fg && back == bg {
                    run.push(ch);
                } else {
                    if !run.is_empty() {
                        spans.push(Span::styled(
                            std::mem::take(&mut run),
                            Style::default().fg(fg).bg(bg),
                        ));
                    }
                    fg = color;
                    bg = back;
                    run.push(ch);
                }
            }
            if !run.is_empty() {
                spans.push(Span::styled(run, Style::default().fg(fg).bg(bg)));
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

fn rgb(color: Color) -> (f64, f64, f64) {
    match color {
        Color::Rgb(r, g, b) => (r as f64, g as f64, b as f64),
        _ => (255.0, 255.0, 255.0),
    }
}

/// 把 `color` 按 `amount`（0=原色，1=背景色）融向背景，用于淡出与拖影
fn fade(color: Color, amount: f64) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    let (r1, g1, b1) = rgb(color);
    let (r2, g2, b2) = rgb(BG);
    Color::Rgb(
        (r1 + (r2 - r1) * amount) as u8,
        (g1 + (g2 - g1) * amount) as u8,
        (b1 + (b2 - b1) * amount) as u8,
    )
}

fn mix(a: Color, b: Color, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (r1, g1, b1) = rgb(a);
    let (r2, g2, b2) = rgb(b);
    Color::Rgb(
        (r1 + (r2 - r1) * t) as u8,
        (g1 + (g2 - g1) * t) as u8,
        (b1 + (b2 - b1) * t) as u8,
    )
}

fn hash(x: u32, y: u32) -> u32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

/// 海面底色：近岸偏青，远处压深，形成纵深感
fn sea_bg(depth: f64) -> Color {
    let shallow = Color::Rgb(12, 46, 62);
    let mid = Color::Rgb(8, 30, 43);
    let deep = Color::Rgb(4, 13, 21);
    if depth < 0.4 {
        mix(shallow, mid, depth / 0.4)
    } else {
        mix(mid, deep, (depth - 0.4) / 0.6)
    }
}

/// 浪花前景色：在底色上提亮，避免整片死黑
fn sea_color(depth: f64) -> Color {
    let base = sea_bg(depth);
    mix(base, ACCENT, 0.45 - depth * 0.2)
}

fn draw_sky(canvas: &mut Canvas, t: f64, horizon: i32) {
    let reveal = (t / 620.0).clamp(0.0, 1.0);
    for y in 0..horizon.max(0) {
        for x in 0..canvas.width {
            let r = hash(x as u32, y as u32);
            if r % 100 < 3 {
                let twinkle = ((t * 0.0022 + (r % 628) as f64 / 100.0).sin() * 0.5 + 0.5) * reveal;
                let ch = match r % 11 {
                    0 => '*',
                    1 | 2 => '+',
                    _ => '·',
                };
                let color = fade(STAR, 0.15 + (1.0 - twinkle) * 0.72);
                canvas.put(x as i32, y, ch, color);
            }
        }
    }

    // 右上角的弦月
    let moon_x = canvas.width as i32 - 16;
    let moon_y = 4;
    let moon = ["  .-.  ", " (   ) ", "  '-'  "];
    for (row, line) in moon.iter().enumerate() {
        canvas.text(moon_x, moon_y + row as i32, line, fade(GOLD, 0.15));
    }
    canvas.put(moon_x + 5, moon_y + 1, '·', fade(GOLD, 0.55));
    canvas.put(moon_x - 1, moon_y + 2, '·', fade(GOLD, 0.7));
    canvas.put(moon_x + 2, moon_y + 3, '·', fade(GOLD, 0.68));
    canvas.put(moon_x + 6, moon_y + 3, '·', fade(GOLD, 0.75));

    // 地平线余晖
    let glow = mix(SKY_DEEP, ACCENT, 0.16);
    for x in 0..canvas.width {
        canvas.put(x as i32, horizon - 1, '▁', glow);
    }
}

fn draw_sea(canvas: &mut Canvas, t: f64, horizon: i32, water_y: i32, front: bool) {
    let height = canvas.height as i32;
    let span = (height - horizon).max(1) as f64;
    let (from, to) = if front {
        (water_y.max(horizon), height)
    } else {
        (horizon, water_y.min(height))
    };
    for y in from..to {
        let depth = ((y - horizon) as f64 / span).clamp(0.0, 1.0);
        let bg = sea_bg(depth);
        let base = sea_color(depth);
        let speed = 2.2 + depth * 5.0;
        for x in 0..canvas.width {
            let xf = x as f64;
            // 每个格子独立的抖动，打散整行同相位造成的“扫描线”
            let n = hash(x as u32, (y.max(0) as u32).wrapping_mul(2_246_822_519));
            let jitter = (n % 97) as f64 * 0.011;
            let phase = xf * 0.42 + y as f64 * 0.9 + jitter - t * 0.001 * speed;
            let wave = phase.sin() * 0.5 + 0.5;
            let sparkle = (n >> 7) % 100;

            let (ch, color) = if y == horizon {
                if wave > 0.74 {
                    ('≈', mix(base, FOAM, 0.30))
                } else if wave > 0.56 {
                    ('~', mix(base, FOAM, 0.12))
                } else {
                    (' ', base)
                }
            } else if wave > 0.93 && sparkle < 74 {
                ('▂', mix(base, FOAM, 0.26))
            } else if wave > 0.78 {
                ('▁', mix(base, FOAM, 0.05))
            } else if wave > 0.62 && sparkle > 58 {
                ('~', mix(base, FOAM, 0.10))
            } else {
                (' ', base)
            };

            canvas.put_bg(x as i32, y, ch, color, bg);
        }
    }
}

fn draw_ship(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    opacity: f64,
    erase: bool,
    t: f64,
    horizon: i32,
    height: i32,
) {
    let len = SHIP_LEN;
    let span = (height - horizon).max(1) as f64;
    // 每一行应有的背景：天空行是夜色，水面行是对应深度的海水色
    let row_bg = |py: i32| -> Color {
        if py < horizon {
            BG
        } else {
            sea_bg((((py - horizon) as f64) / span).clamp(0.0, 1.0))
        }
    };
    if erase {
        // 清空包围盒，避免星空和水纹穿进船体内部
        for row in 0..SHIP_HEIGHT {
            let py = y + row;
            let bg = row_bg(py);
            for col in 0..len {
                canvas.put_bg(x + col, py, ' ', bg, bg);
            }
        }
    }
    // 烟囱黑烟（先画，藏在船体之后）
    for i in 0..4i32 {
        let life = ((t * 0.0009 + i as f64 * 0.27) % 1.0) as f64;
        let sx = x + 34 - (life * 6.0) as i32;
        let sy = y - 2 - (life * 3.5) as i32;
        canvas.put(
            sx,
            sy,
            if i % 2 == 0 { '·' } else { '~' },
            fade(MUTED, 0.25 + life * 0.55),
        );
    }

    let tint = |row: i32| -> f64 {
        match row {
            0 => 0.16,
            1 => 0.06,
            2 => 0.0,
            3 => 0.24,
            4 => 0.42,
            _ => 0.6,
        }
    };
    let mut cell = |col: i32, row: i32, ch: char, color: Color| {
        let py = y + row;
        let bg = row_bg(py);
        let shade = fade(color, tint(row));
        if opacity >= 1.0 {
            canvas.put_bg(x + col, py, ch, shade, bg);
        } else {
            // 拖影只给已有水纹染色，保留海面质感
            canvas.tint(x + col, py, shade, opacity * 0.9);
        }
    };

    // 舰桥（第 0-1 行）
    for col in 12..=26 {
        cell(col, 0, '▄', GOLD);
        cell(col, 1, '█', GOLD);
    }
    for col in (15..=23).step_by(2) {
        cell(col, 1, '▪', ACCENT);
    }

    // 烟囱（第 0-1 行）
    for col in 32..=36 {
        cell(col, 0, '█', GOLD_DIM);
        cell(col, 1, '█', GOLD_DIM);
    }

    // 甲板：半高亮线，让船体显得修长
    for col in 1..=len - 2 {
        cell(col, 2, '▄', GOLD);
    }

    // 船体上缘
    for col in 0..=len - 4 {
        cell(col, 3, '█', GOLD_DIM);
    }
    // 船体下缘（含舷窗），船首逐行内收形成斜艏
    for col in 0..=len - 7 {
        if col % 5 == 2 && col > 3 {
            cell(col, 4, 'o', ACCENT);
        } else {
            cell(col, 4, '█', mix(GOLD_DIM, BG, 0.42));
        }
    }
    // 龙骨
    for col in 2..=len - 12 {
        cell(col, 5, '▀', mix(GOLD_DIM, BG, 0.66));
    }
}

fn ship_width() -> i32 {
    SHIP_LEN
}

/// 船体右端（船首）的列偏移
fn bow_offset() -> i32 {
    SHIP_LEN - 3
}

fn draw_spray(canvas: &mut Canvas, bow_x: i32, water_y: i32, t: f64, force: f64) {
    if force <= 0.02 {
        return;
    }
    // 飞沫从水线抛出，白冠贴着船艏吃水处
    let top = water_y - 1;

    // ① 主飞沫：紧贴船首向上前方抛射，越接近顶点越亮
    let count = (force * 92.0) as i32;
    for i in 0..count {
        let seed = hash(i as u32 * 13 + 5, (t / 38.0) as u32);
        let noise = (seed % 1000) as f64 / 1000.0;
        let life = ((t * 0.0023 + noise) % 1.0).clamp(0.0, 1.0);
        let spread = (seed % 9) as f64 - 4.0;
        let dx = life * 6.5 + spread * 0.5 - 1.0;
        let rise = life * (1.0 - life) * 4.0; // 0 → 1 → 0
        let dy = (-rise * 9.0 - noise * 1.6).max(-5.5);
        let ch = match seed % 7 {
            0 => '*',
            1 | 2 => '·',
            3 => '\\',
            4 => '/',
            5 => '~',
            _ => '·',
        };
        let bright = (0.12 + (1.0 - life) * 0.5 + force * 0.22).min(0.9);
        canvas.paint(bow_x + dx as i32, top + dy as i32, ch, fade(FOAM, bright));
    }

    // ② 贴水水雾：船首前方的低空碎沫
    let mist = (force * 26.0) as i32;
    for i in 0..mist {
        let seed = hash(i as u32 * 29 + 3, (t / 52.0) as u32);
        let dx = 1 + (seed % 11) as i32;
        let dy = -(((seed >> 4) % 2) as i32);
        canvas.paint(
            bow_x + dx,
            water_y + dy,
            if seed % 3 == 0 { '·' } else { '~' },
            fade(FOAM, 0.35 + (seed % 40) as f64 / 100.0),
        );
    }

    // ③ 船艏吃水处翻滚的白沫
    let crown = (force * 22.0) as i32;
    for i in 0..crown {
        let spread = (i as f64 / crown.max(1) as f64 - 0.5) * 14.0;
        let ch = match i % 3 {
            0 => '▁',
            1 => '≈',
            _ => '~',
        };
        canvas.paint(
            bow_x - 5 + spread as i32,
            water_y + 1 + i % 2,
            ch,
            fade(FOAM, 0.28 + (i % 3) as f64 * 0.2),
        );
    }

    // ④ 船首上冲的碎沫：贴着船艏向上拉出一条纵向水线
    let jet = (force * 8.0) as i32;
    for i in 0..jet {
        let seed = hash(i as u32 * 31 + 17, (t / 60.0) as u32);
        let y = water_y - 1 - i / 2 - (seed % 2) as i32;
        let x = bow_x + (seed % 3) as i32 - 1;
        canvas.paint(
            x,
            y,
            if i % 2 == 0 { '·' } else { '~' },
            fade(FOAM, 0.2 + i as f64 * 0.05),
        );
    }
}

fn draw_wake(canvas: &mut Canvas, stern_x: i32, water_y: i32, t: f64, force: f64) {
    let length = (force * 44.0) as i32;
    for i in 0..length {
        let flicker = ((t * 0.013 + i as f64 * 0.55).sin() * 0.5 + 0.5) * force;
        if flicker > 0.34 {
            let color = fade(FOAM, 0.2 + (i as f64 / length.max(1) as f64) * 0.6);
            canvas.paint(
                stern_x - i,
                water_y + if i % 5 == 0 { 1 } else { 0 },
                if flicker > 0.72 { '≈' } else { '~' },
                color,
            );
        }
    }

    // 高速水线：冲刺时船尾拉出细长白线
    if force > 0.45 {
        let streak = ((force - 0.45) * 48.0) as i32;
        for i in 0..streak {
            let seed = hash(i as u32 * 17 + 9, (t / 40.0) as u32);
            canvas.paint(
                stern_x - 4 - (seed % 32) as i32,
                water_y - 1,
                '-',
                fade(FOAM, 0.42 + (seed % 20) as f64 / 100.0),
            );
        }
    }
}

fn draw_bow_wave(canvas: &mut Canvas, bow_x: i32, water_y: i32, force: f64) {
    if force < 0.2 {
        return;
    }
    let reach = (force * 14.0).max(2.0) as i32;
    for i in 0..reach {
        let x = bow_x + 1 + i;
        let y = water_y + 1 - (i as f64 * 0.42) as i32;
        let ch = match i % 4 {
            0 => '~',
            1 => '·',
            2 => '/',
            _ => '≈',
        };
        canvas.paint(x, y, ch, fade(FOAM, 0.14 + i as f64 * 0.05));
    }
    // 船艏吃水处的翻滚白沫
    let foam = (force * 10.0) as i32;
    for i in 0..foam {
        canvas.paint(
            bow_x - 6 + i,
            water_y + 1 + i % 2,
            if i % 3 == 0 { '▁' } else { '≈' },
            fade(FOAM, 0.15 + (i % 4) as f64 * 0.12),
        );
    }
}

fn draw_title(canvas: &mut Canvas, t: f64, title_y: i32) {
    if t < T_CHARGE_START {
        return;
    }
    // 清空标题带，避免星点穿插导致标题断字
    for y in title_y..=(title_y + 5) {
        for x in 0..canvas.width {
            canvas.put(x as i32, y, ' ', BG);
        }
    }
    let letters = ['S', 'H', 'I', 'P', 'W', 'I', 'Z'];
    let spacing = 3usize;
    let total = letters.len() + (letters.len() - 1) * spacing;
    let start_x = (canvas.width as i32 - total as i32) / 2;

    for (index, letter) in letters.iter().enumerate() {
        let local =
            ((t - T_TITLE_START - index as f64 * T_TITLE_STEP) / T_TITLE_FADE).clamp(0.0, 1.0);
        let x = start_x + (index * (spacing + 1)) as i32;
        if local <= 0.0 {
            canvas.put(x, title_y, '·', fade(MUTED, 0.55));
            continue;
        }
        let color = mix(MUTED, GOLD, local);
        canvas.put(x, title_y, *letter, color);
    }

    // 标题下方的浪纹（随标题一起浮现）
    let underline = ((t - T_TITLE_START - 260.0) / 520.0).clamp(0.0, 1.0);
    if underline > 0.0 {
        let line_y = title_y + 1;
        let line_start = start_x - 3;
        for i in 0..(total as i32 + 6) {
            let wave = ((i as f64 * 0.9 - t * 0.006).sin() * 0.5 + 0.5) * 0.55 + 0.25;
            canvas.put(
                line_start + i,
                line_y,
                if i % 2 == 0 { '≈' } else { '~' },
                fade(ACCENT, 1.0 - wave * underline),
            );
        }
    }

    // 副标题
    let subtitle = "舰 队 指 挥 · 终 端";
    let subtitle_x = (canvas.width as i32 - subtitle.chars().count() as i32) / 2;
    let sub_local = ((t - T_TITLE_START - 900.0) / 500.0).clamp(0.0, 1.0);
    if sub_local > 0.0 {
        canvas.text(
            subtitle_x,
            title_y + 3,
            subtitle,
            fade(MUTED, 1.0 - sub_local * 0.35),
        );
    }

    // 版本号
    let version = format!("v{}  ·  终端舰队指挥台", env!("CARGO_PKG_VERSION"));
    let version_x = (canvas.width as i32 - version.chars().count() as i32) / 2;
    let ver_local = ((t - T_TITLE_START - 1200.0) / 500.0).clamp(0.0, 1.0);
    if ver_local > 0.0 {
        canvas.text(
            version_x,
            title_y + 5,
            &version,
            fade(MUTED, 1.0 - ver_local * 0.5),
        );
    }
}

fn render(width: u16, height: u16, t: f64) -> Vec<Line<'static>> {
    let mut canvas = Canvas::new(width as usize, height as usize);
    let h = height as i32;
    let w = width as i32;
    let horizon = ((h as f64) * 0.58).round() as i32;
    let water_y = horizon + 3;

    draw_sky(&mut canvas, t, horizon);
    draw_sea(&mut canvas, t, horizon, water_y, false);

    // 冲刺进度与速度：起步即全速，临近入港急刹
    let charging = t >= T_CHARGE_START;
    let progress = if charging {
        ((t - T_CHARGE_START) / (T_CHARGE_END - T_CHARGE_START)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let eased = 1.0 - (1.0 - progress).powi(4);
    let target_x = (w as f64 * 0.5) - ship_width() as f64 * 0.56;
    let start_x = -(ship_width() as f64) - 20.0;
    let ship_x = (start_x + (target_x - start_x) * eased) as i32;
    // 浪花强度：前半程迅速拉满，入港瞬间收束
    let force = if charging {
        (progress * std::f64::consts::PI).sin().powf(0.62)
    } else {
        0.0
    };
    let idle = if progress >= 1.0 { 0.3 } else { 0.0 };

    // 抛锚后的轻微起伏，叠加入港顿挫
    let mut bob = if progress >= 1.0 {
        ((t * 0.0034).sin() * 0.5 + 0.5).round() as i32
    } else {
        0
    };
    if progress >= 1.0 {
        let since = t - T_CHARGE_END;
        if since < 420.0 {
            let phase = since / 420.0 * std::f64::consts::PI * 2.4;
            bob += (phase.sin() * (-since / 130.0).exp() * 1.7) as i32;
        }
    }
    let ship_y = water_y - SHIP_HEIGHT + 1 + bob;

    // 冲刺拖影
    if force > 0.1 {
        draw_ship(
            &mut canvas,
            ship_x - 5,
            ship_y + 1,
            force * 0.2,
            false,
            t,
            horizon,
            h,
        );
        draw_ship(
            &mut canvas,
            ship_x - 11,
            ship_y + 1,
            force * 0.12,
            false,
            t,
            horizon,
            h,
        );
        draw_ship(
            &mut canvas,
            ship_x - 19,
            ship_y + 2,
            force * 0.05,
            false,
            t,
            horizon,
            h,
        );
    }
    draw_ship(&mut canvas, ship_x, ship_y, 1.0, true, t, horizon, h);

    // 船前的近处浪层最后叠加，形成船体吃水的效果
    draw_sea(&mut canvas, t, horizon, water_y, true);

    let bow_x = ship_x + bow_offset();
    draw_bow_wave(&mut canvas, bow_x, water_y, force.max(idle));
    draw_wake(&mut canvas, ship_x - 1, water_y, t, force.max(idle * 1.2));
    if charging {
        draw_spray(&mut canvas, bow_x, water_y, t, force);
    }

    draw_title(&mut canvas, t, (horizon - 13).max(1));

    canvas.lines()
}

/// 在已进入全屏的终端上播放启动动画：播完自动返回，不等待按键
pub fn play(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let started = Instant::now();
    loop {
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let t = elapsed.min(DURATION_MS);
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Paragraph::new(render(area.width, area.height, t)), area);
        })?;
        // 动画期间吞掉误触按键，避免带进主界面
        if event::poll(Duration::from_millis(16))? {
            let _ = event::read()?;
        }
        if elapsed >= DURATION_MS {
            return Ok(());
        }
    }
}

/// 独立预览：自己独占画布，播完自动退出；`hold` 为定格时间点（毫秒）
pub fn run_standalone(hold: Option<f64>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::cursor::Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;

    let result = match hold {
        Some(stamp) => preview_hold(&mut terminal, stamp),
        None => play(&mut terminal),
    };

    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )
    .ok();
    terminal.show_cursor().ok();
    result
}

fn preview_hold(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    stamp: f64,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Paragraph::new(render(area.width, area.height, stamp)), area);
        })?;
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    return Ok(());
                }
            }
        }
    }
}

/// 纯文本帧预览：不进入全屏，直接打印关键时间点，便于审阅画面构成
pub fn dump_frames() {
    let stamps = [
        100.0, 300.0, 520.0, 760.0, 980.0, 1180.0, 1500.0, 2200.0, 3200.0,
    ];
    for stamp in stamps {
        println!("===== t = {stamp:.0} ms =====");
        for line in render(100, 30, stamp) {
            let text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            println!("{text}");
        }
        println!();
    }
}
