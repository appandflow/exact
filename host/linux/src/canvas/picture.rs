//! What a reader copies a picture from: a decoded image, or a canvas's
//! rasterized drawing (Canvas 2D, LLP 1056), both premultiplied RGBA.
use crate::image::Bitmap;
use std::sync::{Arc, Weak};
use tiny_skia::Pixmap;

/// A picture the reader fetches by id ([`super::CanvasHost::image`]).
#[derive(Clone)]
pub enum Picture {
    /// A decoded image.
    Bitmap(Arc<Bitmap>),
    /// A canvas's pixels as last drawn.
    Canvas(Arc<Pixmap>),
}

impl Picture {
    /// Pixels across.
    pub fn width(&self) -> u32 {
        match self {
            Picture::Bitmap(b) => b.width(),
            Picture::Canvas(p) => p.width(),
        }
    }

    /// Pixels down.
    pub fn height(&self) -> u32 {
        match self {
            Picture::Bitmap(b) => b.height(),
            Picture::Canvas(p) => p.height(),
        }
    }

    /// The allocation it is known by.
    pub(super) fn key(&self) -> usize {
        match self {
            Picture::Bitmap(b) => Arc::as_ptr(b) as usize,
            Picture::Canvas(p) => Arc::as_ptr(p) as usize,
        }
    }

    pub(super) fn downgrade(&self) -> WeakPicture {
        match self {
            Picture::Bitmap(b) => WeakPicture::Bitmap(Arc::downgrade(b), b.shared()),
            Picture::Canvas(p) => WeakPicture::Canvas(Arc::downgrade(p)),
        }
    }
}

impl AsRef<[u8]> for Picture {
    fn as_ref(&self) -> &[u8] {
        match self {
            Picture::Bitmap(b) => b.as_ref().as_ref(),
            Picture::Canvas(p) => p.data(),
        }
    }
}

/// A picture the recorder announced, held weakly: dropped by the presenter,
/// it is freed on the reader too.
pub(super) enum WeakPicture {
    /// The image, and whether the reader draws from its own pixels.
    Bitmap(Weak<Bitmap>, bool),
    Canvas(Weak<Pixmap>),
}

impl WeakPicture {
    pub(super) fn alive(&self) -> bool {
        match self {
            WeakPicture::Bitmap(w, _) => w.strong_count() > 0,
            WeakPicture::Canvas(w) => w.strong_count() > 0,
        }
    }

    /// Whether the reader holds no copy of its own (it draws from the
    /// picture's GPU buffer): nothing is freed by letting go of it early.
    pub(super) fn shared(&self) -> bool {
        matches!(self, WeakPicture::Bitmap(_, true))
    }

    /// Whether this is `picture`'s allocation, still alive.
    pub(super) fn is(&self, picture: &Picture) -> bool {
        match (self, picture) {
            (WeakPicture::Bitmap(w, _), Picture::Bitmap(b)) => {
                w.upgrade().is_some_and(|live| Arc::ptr_eq(&live, b))
            }
            (WeakPicture::Canvas(w), Picture::Canvas(p)) => {
                w.upgrade().is_some_and(|live| Arc::ptr_eq(&live, p))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{stream, Recorder, FINISHED, IMAGE, IMAGE_DEF, IMAGE_RRECT};
    use super::*;
    use crate::paint::Backend;

    /// A child placed by a `ChildrenMode::Each` surface reaches the reader as
    /// a picture over the box the warp gave it, as a Canvas 2D bitmap does.
    #[test]
    fn a_placed_canvas_child_is_a_picture_in_the_stream() {
        let mut r = Recorder::new();
        r.begin(400., 800., 2.);
        r.surface_image(Arc::new(Pixmap::new(8, 6).unwrap()), (10., 20., 4., 3.));
        r.finish().unwrap();
        let ops = FINISHED.with(|f| f.borrow_mut().take()).unwrap_or_default();
        let (mut defined, mut drawn) = (None, None);
        let mut at = 0;
        while at < ops.len() {
            match ops[at] {
                IMAGE_DEF => defined = Some((ops[at + 1], ops[at + 2], ops[at + 3])),
                // Both draw the picture over the four floats after its id.
                IMAGE | IMAGE_RRECT => {
                    let dst: Vec<f32> = ops[at + 2..at + 6]
                        .iter()
                        .map(|w| f32::from_bits(*w))
                        .collect();
                    drawn = Some((ops[at + 1], dst));
                }
                _ => {}
            }
            at = stream::end(&ops, at).unwrap();
        }
        let (id, w, h) = defined.expect("the picture is defined");
        assert_eq!((w, h), (8, 6));
        assert_eq!(drawn, Some((id, vec![10., 20., 4., 3.])));
    }
}
