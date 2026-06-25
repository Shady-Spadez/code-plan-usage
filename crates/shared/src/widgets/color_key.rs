/// Query the OpenGL framebuffer's alpha channel bit depth.
/// Returns -1 if the GL context is unavailable.
#[cfg(windows)]
pub fn query_framebuffer_alpha(gl: &eframe::glow::Context) -> i32 {
    use eframe::glow::HasContext as _;
    unsafe {
        gl.get_framebuffer_attachment_parameter_i32(
            eframe::glow::FRAMEBUFFER,
            eframe::glow::COLOR_ATTACHMENT0,
            eframe::glow::FRAMEBUFFER_ATTACHMENT_ALPHA_SIZE,
        )
    }
}

/// Stub for non-Windows platforms.
#[cfg(not(windows))]
pub fn query_framebuffer_alpha(_gl: &eframe::glow::Context) -> i32 {
    -1
}
