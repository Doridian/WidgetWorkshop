// Palette lookup and scaling of the 8-bit framebuffer, with a true-colour
// overlay (a film) on top.

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

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(framebuffer));
    let p = floor((pos.xy - view.offset) / view.scale);
    if (p.x < 0.0 || p.y < 0.0 || p.x >= size.x || p.y >= size.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let o = p - view.overlay.xy;
    if (o.x >= 0.0 && o.y >= 0.0 && o.x < view.overlay.z && o.y < view.overlay.w) {
        return textureLoad(overlay, vec2<i32>(o), 0);
    }
    let index = textureLoad(framebuffer, vec2<i32>(p), 0).r;
    return textureLoad(palette, vec2<i32>(i32(index), 0), 0);
}
