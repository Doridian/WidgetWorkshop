// Palette lookup and scaling of the 8-bit framebuffer, with a true-colour
// overlay (a film) on top. At a scale that isn't whole, a window pixel
// that falls on the edge between two framebuffer pixels blends them by
// how much of it each covers ("sharp bilinear"), so every pixel comes out
// the same size; at a whole scale none does.

struct View {
    // Top left of the framebuffer in the window, in pixels.
    offset: vec2<f32>,
    // Window pixels per framebuffer pixel.
    scale: f32,
    _pad: f32,
    // The overlay's left, top, width and height in the framebuffer.
    overlay: vec4<f32>,
};

@group(0) @binding(0) var framebuffer: texture_2d<u32>;
@group(0) @binding(1) var palette: texture_2d<f32>;
@group(0) @binding(2) var<uniform> view: View;
@group(0) @binding(3) var overlay: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // One triangle that covers the window.
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// The colour of framebuffer pixel `p`, the overlay's where it covers it.
fn texel(p: vec2<i32>) -> vec4<f32> {
    let o = vec2<f32>(p) - view.overlay.xy;
    if (o.x >= 0.0 && o.y >= 0.0 && o.x < view.overlay.z && o.y < view.overlay.w) {
        return textureLoad(overlay, vec2<i32>(o), 0);
    }
    let index = textureLoad(framebuffer, p, 0).r;
    return textureLoad(palette, vec2<i32>(i32(index), 0), 0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(framebuffer));
    let q = (pos.xy - view.offset) / view.scale;
    if (q.x < 0.0 || q.y < 0.0 || q.x >= size.x || q.y >= size.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    // The pixels whose centres are around `q`, and how far towards the
    // second the window pixel's own area reaches.
    let t = q - 0.5;
    let a = floor(t);
    let w = clamp((t - a - 0.5) * view.scale + 0.5, vec2<f32>(0.0), vec2<f32>(1.0));
    let last = vec2<i32>(size) - 1;
    let p0 = clamp(vec2<i32>(a), vec2<i32>(0), last);
    let p1 = clamp(vec2<i32>(a) + 1, vec2<i32>(0), last);
    let top = mix(texel(p0), texel(vec2<i32>(p1.x, p0.y)), w.x);
    let bottom = mix(texel(vec2<i32>(p0.x, p1.y)), texel(p1), w.x);
    return mix(top, bottom, w.y);
}
