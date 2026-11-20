//! LLP 1066: `background-image` gradients held to Chrome's pixels. The page
//! is `scripts/fixtures/gradients.contract`; Chrome's pictures of it (the CSS
//! declarations at 1×) are `gradients.web.png` as booted and `gradients.web-dark.png`
//! after `dark`. Each case's box is compared within the border parity's band.

use crate::borders::{boot, held_to_chrome, held_to_chrome_with_band, view};

const CASES: [&str; 20] = [
    "linear",
    "angle",
    "corner",
    "fade",
    "over",
    "clip",
    "radial",
    "circle",
    "ellipse",
    "rounded",
    "bordered",
    "scheme",
    "hatch",
    "length",
    "mixed",
    "zero-two",
    "zero-three",
    "zero-four",
    "radii",
    "radius",
];

/// The radial cases, which every painter samples at pixel centres as CSS
/// does: within a tighter band than the antialiased edges need.
const RADIAL: [&str; 5] = ["radial", "circle", "ellipse", "radii", "radius"];

#[test]
fn every_gradient_case_matches_chrome_light_then_dark() {
    let choices = crate::paint::painters();
    let mut failures = Vec::new();
    for choice in choices {
        let name = format!("{choice:?}");
        let cases = CASES;
        let mut p = boot(choice, "gradients.contract");
        let _ = p.resize(390.0, 650.0);
        failures.extend(held_to_chrome(&mut p, "gradients.web.png", &name, &cases));
        failures.extend(held_to_chrome_with_band(
            &mut p,
            "gradients.web.png",
            &name,
            &RADIAL,
            0.5,
        ));
        // `light-dark()` stops follow the scheme, and so does the page
        // under the transparent ones.
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(<()>::default);
        failures.extend(held_to_chrome(
            &mut p,
            "gradients.web-dark.png",
            &name,
            &cases,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// CSS `background-blend-mode` (`background-blend.contract`), held to
/// Chrome's picture of it (`background-blend.web.png`, the web host at 1×).
/// The multiplied hatch's stripes are the colour, darker, and nothing of the
/// red box behind a blended background reaches it.
#[test]
fn background_blend_mode_matches_chrome() {
    use crate::borders::view;
    let cases = ["multiply", "normal", "clear", "screen", "linear"];
    let mut failures = Vec::new();
    for choice in crate::paint::painters() {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "background-blend.contract");
        failures.extend(held_to_chrome(
            &mut p,
            "background-blend.web.png",
            &name,
            &cases,
        ));
        let rects: Vec<_> = ["multiply", "normal", "clear"]
            .map(|case| {
                let id = view(&p, case);
                (case, p.boxes().iter().find(|b| b.id == id).unwrap().rect)
            })
            .into();
        let frame = p.frame();
        let share = |case: &str, rgb: [u8; 3]| {
            let (x, y, w, h) = rects.iter().find(|(c, _)| *c == case).unwrap().1;
            let mut near = 0;
            for py in y as u32..(y + h) as u32 {
                for px in x as u32..(x + w) as u32 {
                    let c = frame.pixel(px, py).unwrap().demultiply();
                    let got = [c.red(), c.green(), c.blue()];
                    near += usize::from(got.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 6));
                }
            }
            near as f64 / (w * h) as f64
        };
        let (stripe, base, grey) = ([34, 98, 181], [42, 122, 226], [204, 204, 204]);
        assert!(
            share("multiply", stripe) > 0.25,
            "{name}: multiplied stripes"
        );
        assert!(
            share("multiply", base) > 0.25,
            "{name}: the colour under white"
        );
        assert_eq!(share("multiply", grey), 0.0, "{name}: no stripe unblended");
        assert!(
            share("normal", grey) > 0.25,
            "{name}: the unblended stripes"
        );
        assert!(share("clear", grey) > 0.25, "{name}: grey over no colour");
        assert_eq!(
            share("clear", [204, 0, 0]),
            0.0,
            "{name}: nothing of the red behind"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
