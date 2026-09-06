//! Embedded mascot at common panel sizes, independent of the installed icon theme.

use std::sync::LazyLock;

pub(super) fn pixmaps() -> Vec<ksni::Icon> {
    static ICONS: LazyLock<Vec<ksni::Icon>> = LazyLock::new(|| {
        // Regenerate these RGBA8 exports with scripts/render-tray-icons.sh.
        let images: &[(i32, &[u8])] = &[
            (16, include_bytes!("../../assets/tray/icon-16.rgba")),
            (20, include_bytes!("../../assets/tray/icon-20.rgba")),
            (22, include_bytes!("../../assets/tray/icon-22.rgba")),
            (24, include_bytes!("../../assets/tray/icon-24.rgba")),
            (32, include_bytes!("../../assets/tray/icon-32.rgba")),
            (48, include_bytes!("../../assets/tray/icon-48.rgba")),
            (64, include_bytes!("../../assets/tray/icon-64.rgba")),
        ];
        images
            .iter()
            .map(|&(size, rgba)| {
                assert_eq!(rgba.len(), (size * size * 4) as usize);
                let mut data = rgba.to_vec();
                // StatusNotifierItem expects bytes in A, R, G, B order.
                for pixel in data.chunks_exact_mut(4) {
                    pixel.rotate_right(1);
                }
                ksni::Icon {
                    width: size,
                    height: size,
                    data,
                }
            })
            .collect()
    });
    ICONS.clone()
}
