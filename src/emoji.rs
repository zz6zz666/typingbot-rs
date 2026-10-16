//! 彩色 emoji 渲染。\n//!
//! egui/epaint 的文字管线只会把字形**轮廓**栅格化成单色掩膜（图集里只存覆盖率，
//! 上屏时乘顶点颜色），COLR v1 的彩色信息它完全拿不到。这里自己补上这一层：
//!
//! 1. `skrifa` 遍历 COLR v1 图层树，录下「路径 + 画刷」（实色 / 线性渐变 / 径向渐变）；
//! 2. `vello_cpu` 按设备像素逐层光栅化成 RGBA 贴图（带缓存）；
//! 3. 排版时把对应字形的单色轮廓顶点改成透明，再把贴图按基线位置盖上去。
//!
//! 只处理单码点 emoji（含 `⚠️` 这类带变体选择符的）；ZWJ 组合序列（👨‍👩‍👧）、旗帜等
//! 保持单色：那些序列整形后是一个连字字形，无法用码点反查。

use std::collections::HashMap;

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use skrifa::{
    MetadataProvider,
    color::{Brush, ColorPainter, ColorStop as ColorStopRef, CompositeMode, Extend, Transform},
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
    prelude::{FontRef, GlyphId},
    raw::{TableProvider, types::BoundingBox},
};
use vello_cpu::{
    color::{AlphaColor, DynamicColor, Srgb},
    kurbo::{Affine, BezPath, Point, Rect},
    peniko::{ColorStop, ColorStops, Gradient, RadialGradientPosition, SweepGradientPosition},
};

/// 一张彩色 emoji 贴图 + 它在字体坐标系里的墨迹包围盒（字体单位，y 轴向上）。
struct EmojiTex {
    handle: TextureHandle,
    bbox: egui::Rect,
}

/// 彩色 emoji 渲染器（带贴图缓存）。
pub struct EmojiRenderer {
    data: Vec<u8>,
    upem: f32,
    /// (字形 id, 像素尺寸) -> 贴图
    cache: HashMap<(u16, u32), EmojiTex>,
}

impl EmojiRenderer {
    /// 从系统 emoji 字体文件加载（找不到就返回 `None`，界面自动退回单色）。
    pub fn load(paths: &[&str]) -> Option<Self> {
        let data = paths.iter().find_map(|p| std::fs::read(p).ok())?;
        let upem = FontRef::new(&data).ok()?.head().ok()?.units_per_em() as f32;
        if upem <= 0.0 {
            return None;
        }
        Some(Self {
            data,
            upem,
            cache: HashMap::new(),
        })
    }

    /// 1 em 等于多少个字体单位。
    pub fn upem(&self) -> f32 {
        self.upem
    }

    /// 该字符是否有彩色字形（有的话界面层就由我们接管绘制）。
    pub fn color_glyph(&self, ch: char) -> Option<u16> {
        let font = FontRef::new(&self.data).ok()?;
        let gid = font.charmap().map(ch)?;
        font.color_glyphs().get(gid)?;
        Some(gid.to_u32() as u16)
    }

    /// 取（必要时生成）彩色贴图，返回贴图与其在字体单位下的墨迹包围盒。
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        ch: char,
        px: f32,
    ) -> Option<(TextureHandle, egui::Rect)> {
        let gid = self.color_glyph(ch)?;
        let px_i = px.round().clamp(8.0, 512.0) as u32;
        let key = (gid, px_i);
        if let Some(tex) = self.cache.get(&key) {
            return Some((tex.handle.clone(), tex.bbox));
        }
        let (image, bbox) = render_color_glyph(&self.data, GlyphId::new(gid as u32), px_i as f32)?;
        let handle = ctx.load_texture(format!("emoji-{gid}-{px_i}"), image, TextureOptions::LINEAR);
        self.cache.insert(
            key,
            EmojiTex {
                handle: handle.clone(),
                bbox,
            },
        );
        Some((handle, bbox))
    }
}

// --------------------------------------------------------------------- 绘制录制

/// 一层要画的东西：一条路径 + 画刷。
enum Paint {
    Solid(Color32),
    Gradient(Box<Gradient>),
}

/// 把一次 COLR 绘制录制成若干「路径 + 画刷」（路径在字体单位下，已应用图层变换）。
struct Recorder<'f, 'c> {
    outlines: skrifa::outline::OutlineGlyphCollection<'f>,
    colors: &'c [Color32],
    transform: Transform,
    stack: Vec<Transform>,
    fills: Vec<(BezPath, Paint)>,
    bbox: Option<Rect>,
}

impl<'f, 'c> Recorder<'f, 'c> {
    fn new(font: &FontRef<'f>, colors: &'c [Color32]) -> Self {
        Self {
            outlines: font.outline_glyphs(),
            colors,
            transform: Transform::default(),
            stack: Vec::new(),
            fills: Vec::new(),
            bbox: None,
        }
    }

    fn grow(&mut self, rect: Rect) {
        if !rect.width().is_finite() || !rect.height().is_finite() {
            return;
        }
        self.bbox = Some(match self.bbox {
            Some(b) => b.union(rect),
            None => rect,
        });
    }

    /// CPAL 调色板取色，再乘上画刷自带的 alpha。
    fn color_of(&self, index: u16, alpha: f32) -> Color32 {
        let c = self
            .colors
            .get(index as usize)
            .copied()
            .unwrap_or(Color32::from_rgb(128, 128, 128));
        let a = (c.a() as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
    }

    /// 把 skrifa 的画刷转成 vello 能画的画刷；渐变几何按图层变换 `t` 一起变换。
    fn paint_of(&self, brush: &Brush<'_>, t: Transform) -> Paint {
        match brush {
            Brush::Solid {
                palette_index,
                alpha,
            } => Paint::Solid(self.color_of(*palette_index, *alpha)),
            Brush::LinearGradient {
                p0,
                p1,
                color_stops,
                extend,
            } => {
                let mut g =
                    Gradient::new_linear(map_point(&t, p0.x, p0.y), map_point(&t, p1.x, p1.y));
                g.extend = map_extend(*extend);
                g.stops = self.stops(color_stops);
                Paint::Gradient(Box::new(g))
            }
            Brush::RadialGradient {
                c0,
                r0,
                c1,
                r1,
                color_stops,
                extend,
            } => {
                // 变换基本是旋转 + 等比缩放，半径按 √|det| 缩放即可。
                let scale = ((t.xx * t.yy - t.xy * t.yx) as f64).abs().sqrt() as f32;
                let mut g = Gradient::default();
                g.kind = RadialGradientPosition::new_two_point(
                    map_point(&t, c0.x, c0.y),
                    *r0 * scale,
                    map_point(&t, c1.x, c1.y),
                    *r1 * scale,
                )
                .into();
                g.extend = map_extend(*extend);
                g.stops = self.stops(color_stops);
                Paint::Gradient(Box::new(g))
            }
            Brush::SweepGradient {
                c0,
                start_angle,
                end_angle,
                color_stops,
                extend,
            } => {
                let mut g = Gradient::default();
                g.kind =
                    SweepGradientPosition::new(map_point(&t, c0.x, c0.y), *start_angle, *end_angle)
                        .into();
                g.extend = map_extend(*extend);
                g.stops = self.stops(color_stops);
                Paint::Gradient(Box::new(g))
            }
        }
    }

    fn stops(&self, stops: &[ColorStopRef]) -> ColorStops {
        let mut out = ColorStops::new();
        for s in stops {
            let c = self.color_of(s.palette_index, s.alpha);
            let color = DynamicColor::from_alpha_color(AlphaColor::<Srgb>::new([
                c.r() as f32 / 255.0,
                c.g() as f32 / 255.0,
                c.b() as f32 / 255.0,
                c.a() as f32 / 255.0,
            ]));
            out.push(ColorStop {
                offset: s.offset,
                color,
            });
        }
        out
    }
}

impl ColorPainter for Recorder<'_, '_> {
    fn push_transform(&mut self, transform: Transform) {
        self.stack.push(self.transform);
        self.transform = compose(self.transform, transform);
    }

    fn pop_transform(&mut self) {
        if let Some(t) = self.stack.pop() {
            self.transform = t;
        }
    }

    // 裁剪盒只是渲染范围提示（ClipList），字形轮廓本身已经限定了范围，忽略。
    fn push_clip_glyph(&mut self, _: GlyphId) {}
    fn push_clip_box(&mut self, _: BoundingBox<f32>) {}
    fn pop_clip(&mut self) {}
    // 合成模式：本字体里只有极少数图层用到，先按普通叠加处理。
    fn push_layer(&mut self, _: CompositeMode) {}
    fn pop_layer(&mut self) {}
    fn fill(&mut self, _brush: Brush<'_>) {}

    fn fill_glyph(
        &mut self,
        glyph_id: GlyphId,
        brush_transform: Option<Transform>,
        brush: Brush<'_>,
    ) {
        let Some(outline) = self.outlines.get(glyph_id) else {
            return;
        };
        let mut pen = PathPen {
            path: BezPath::new(),
            transform: self.transform,
        };
        let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
        if outline.draw(settings, &mut pen).is_err() {
            return;
        }
        self.grow(pen.path.control_box());
        if pen.path.elements().is_empty() {
            return;
        }
        let brush_space = match brush_transform {
            Some(bt) => compose(self.transform, bt),
            None => self.transform,
        };
        let paint = self.paint_of(&brush, brush_space);
        self.fills.push((pen.path, paint));
    }
}

/// 把轮廓写成 kurbo 路径，同时应用 COLR 的变换（仍在字体单位下）。
struct PathPen {
    path: BezPath,
    transform: Transform,
}

impl PathPen {
    fn map(&self, x: f32, y: f32) -> (f64, f64) {
        let t = &self.transform;
        (
            (t.xx * x + t.xy * y + t.dx) as f64,
            (t.yx * x + t.yy * y + t.dy) as f64,
        )
    }
}

impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.path.move_to(p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.path.line_to(p);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let c = self.map(cx, cy);
        let p = self.map(x, y);
        self.path.quad_to(c, p);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let c0 = self.map(cx0, cy0);
        let c1 = self.map(cx1, cy1);
        let p = self.map(x, y);
        self.path.curve_to(c0, c1, p);
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

/// 只求包围盒的 pen，用于把单色基础字形也算进范围。
#[derive(Default)]
struct BBoxPen {
    bbox: Option<Rect>,
}

impl BBoxPen {
    fn grow(&mut self, x: f32, y: f32) {
        let p = Point::new(x as f64, y as f64);
        self.bbox = Some(match self.bbox {
            Some(b) => b.union(Rect::new(p.x, p.y, p.x, p.y)),
            None => Rect::new(p.x, p.y, p.x, p.y),
        });
    }
}

impl OutlinePen for BBoxPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.grow(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.grow(x, y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.grow(cx, cy);
        self.grow(x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.grow(cx0, cy0);
        self.grow(cx1, cy1);
        self.grow(x, y);
    }
    fn close(&mut self) {}
}

/// 矩阵乘法（先应用 `b`，再应用 `a`）。
fn compose(a: Transform, b: Transform) -> Transform {
    let mut out = Transform::default();
    out.xx = a.xx * b.xx + a.xy * b.yx;
    out.xy = a.xx * b.xy + a.xy * b.yy;
    out.yx = a.yx * b.xx + a.yy * b.yx;
    out.yy = a.yx * b.xy + a.yy * b.yy;
    out.dx = a.xx * b.dx + a.xy * b.dy + a.dx;
    out.dy = a.yx * b.dx + a.yy * b.dy + a.dy;
    out
}

fn map_point(t: &Transform, x: f32, y: f32) -> Point {
    Point::new(
        (t.xx * x + t.xy * y + t.dx) as f64,
        (t.yx * x + t.yy * y + t.dy) as f64,
    )
}

fn map_extend(extend: Extend) -> vello_cpu::peniko::Extend {
    match extend {
        Extend::Pad => vello_cpu::peniko::Extend::Pad,
        Extend::Repeat => vello_cpu::peniko::Extend::Repeat,
        Extend::Reflect => vello_cpu::peniko::Extend::Reflect,
        _ => vello_cpu::peniko::Extend::Pad,
    }
}

// --------------------------------------------------------------------- 光栅化

/// 把某个彩色字形渲染成 RGBA 贴图，返回图像与它在字体单位下的墨迹包围盒。
fn render_color_glyph(data: &[u8], gid: GlyphId, px: f32) -> Option<(ColorImage, egui::Rect)> {
    let font = FontRef::new(data).ok()?;
    let upem = font.head().ok()?.units_per_em() as f32;
    let glyph = font.color_glyphs().get(gid)?;
    let palettes = font.color_palettes();
    let palette = palettes.get(0)?;
    let colors: Vec<Color32> = palette
        .colors()
        .iter()
        .map(|c| Color32::from_rgba_unmultiplied(c.red(), c.green(), c.blue(), c.alpha()))
        .collect();

    let mut rec = Recorder::new(&font, &colors);
    glyph.paint(LocationRef::default(), &mut rec).ok()?;

    // 单色基础字形也纳入范围，避免彩色图层比轮廓大时被裁掉。
    if let Some(outline) = font.outline_glyphs().get(gid) {
        let mut pen = BBoxPen::default();
        let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
        if outline.draw(settings, &mut pen).is_ok() {
            if let Some(b) = pen.bbox {
                rec.grow(b);
            }
        }
    }

    let bbox = rec.bbox?;
    if rec.fills.is_empty() || bbox.width() <= 0.5 || bbox.height() <= 0.5 {
        return None;
    }

    let scale = (px / upem) as f64;
    let w = ((bbox.width() * scale).ceil() as u32).clamp(1, 1024) as u16;
    let h = ((bbox.height() * scale).ceil() as u32).clamp(1, 1024) as u16;

    let mut ctx = vello_cpu::RenderContext::new(w, h);
    // 字体单位 -> 设备像素：先平移到包围盒，再缩放并翻转 y 轴（字体 y 向上、屏幕 y 向下）。
    ctx.set_transform(Affine::new([
        scale,
        0.0,
        0.0,
        -scale,
        -bbox.x0 * scale,
        bbox.y1 * scale,
    ]));
    for (path, paint) in &rec.fills {
        match paint {
            Paint::Solid(c) => ctx.set_paint(AlphaColor::<Srgb>::new([
                c.r() as f32 / 255.0,
                c.g() as f32 / 255.0,
                c.b() as f32 / 255.0,
                c.a() as f32 / 255.0,
            ])),
            Paint::Gradient(g) => ctx.set_paint(g.as_ref().clone()),
        }
        ctx.fill_path(path);
    }

    let mut pixmap = vello_cpu::Pixmap::new(w, h);
    let mut resources = vello_cpu::Resources::new();
    ctx.render(&mut pixmap, &mut resources);

    let raw = pixmap.data_as_u8_slice();
    let mut pixels = Vec::with_capacity(w as usize * h as usize);
    for p in raw.chunks_exact(4) {
        pixels.push(Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]));
    }

    let bbox = egui::Rect::from_min_max(
        egui::pos2(bbox.x0 as f32, bbox.y0 as f32),
        egui::pos2(bbox.x1 as f32, bbox.y1 as f32),
    );
    Some((ColorImage::new([w as usize, h as usize], pixels), bbox))
}

// --------------------------------------------------------------------- 界面接入

/// 在排好版的 galley 里找出可以用彩色贴图替代的字形：
/// 记录它们的基线起点（galley 坐标系），并把原来那层单色轮廓的顶点改成透明。
///
/// `text` 是原始文本，用来识别不宜替换的组合（ZWJ 连字、旗帜、肤色修饰符等）。
pub fn collect_marks(
    galley: &mut egui::Galley,
    text: &str,
    emoji: &EmojiRenderer,
    out: &mut Vec<(char, egui::Pos2)>,
) {
    let skip = skip_in_color(text);
    for placed in &mut galley.rows {
        let row_pos = placed.pos;
        let row = std::sync::Arc::make_mut(&mut placed.row);
        let count = row.glyphs.len();
        let vertex_end = row.visuals.glyph_vertex_range.end;
        let vertex_len = row.visuals.mesh.vertices.len();
        for i in 0..count {
            let ch = row.glyphs[i].chr;
            if skip.contains(&ch) || emoji.color_glyph(ch).is_none() {
                continue;
            }
            out.push((ch, row_pos + row.glyphs[i].pos.to_vec2()));
            // 每个字形在网格里占 [first_vertex, 下一个字形的 first_vertex) 一段顶点。
            let start = row.glyphs[i].first_vertex as usize;
            let stop = if i + 1 < count {
                row.glyphs[i + 1].first_vertex as usize
            } else {
                vertex_end
            }
            .min(vertex_len);
            if start < stop {
                for vertex in &mut row.visuals.mesh.vertices[start..stop] {
                    vertex.color = egui::Color32::TRANSPARENT;
                }
            }
        }
    }
}

/// 不应由彩色贴图替代的字符：
/// - 变体选择符 / ZWJ / 键帽组合符 / 肤色修饰符：本身不是独立图形；
/// - 区域指示符（旗帜由两个码点组成）与其后跟 ZWJ 的字形：整形后是一个连字，
///   用单个码点反查会画错，干脆保持单色。
fn skip_in_color(text: &str) -> std::collections::HashSet<char> {
    let mut skip = std::collections::HashSet::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if matches!(c, '\u{200D}' | '\u{FE0F}' | '\u{FE0E}' | '\u{20E3}') {
            skip.insert(c);
        }
        if ('\u{1F3FB}'..='\u{1F3FF}').contains(&c) {
            // 肤色修饰符
            skip.insert(c);
        }
        if ('\u{1F1E6}'..='\u{1F1FF}').contains(&c) || chars.get(i + 1) == Some(&'\u{200D}') {
            skip.insert(c);
        }
    }
    skip
}
