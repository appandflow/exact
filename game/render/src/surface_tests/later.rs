//! Looks registered at tick zero and not shown (issue #129): under the live
//! clock the first frame waits only for what the world presents, the rest is
//! prepared after it, and textures are written off the presenting thread.
use super::cosmetics::{hue, quad, solid};
use super::*;

const LOOKS: [(&str, &str, [u8; 3]); 3] = [
    ("red.model", "red.tex", [255, 0, 0]),
    ("green.model", "green.tex", [0, 255, 0]),
    ("blue.model", "blue.tex", [0, 0, 255]),
];
struct Looks;
impl Game for Looks {
    const ID: &'static str = "registered-looks";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        for (model, texture, _) in LOOKS {
            let mesh = w.generated_model(model, quad(texture)).unwrap();
            if model == "red.model" {
                w.spawn_named("die", (Transform::default(), mesh));
            }
        }
        w.spawn((Transform::at(0., 0., 4.), Camera::default()));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
fn delivered(clock_seekable: bool) -> WorldSurface<Looks, crate::ModelExecutor, true> {
    let mut s = WorldSurface::<Looks, crate::ModelExecutor, true>::default();
    s.device_ready(exact_gpu::wgpu::Features::empty());
    s.bind(&[], None).unwrap();
    s.clock(clock_seekable);
    let mut requested = s.assets().requests;
    requested.sort();
    assert_eq!(requested, ["blue.tex", "green.tex", "red.tex"]);
    for (_, texture, rgb) in LOOKS {
        s.asset(texture, Ok(&solid(rgb)));
    }
    s
}
fn prepared(s: &WorldSurface<Looks, crate::ModelExecutor, true>) -> Vec<&'static str> {
    let sim = s.sim().unwrap();
    LOOKS
        .iter()
        .flat_map(|(model, texture, _)| [*model, *texture])
        .filter(|name| sim.is_prepared(name))
        .collect()
}

#[test]
fn a_live_first_frame_waits_only_for_the_presented_look() {
    let Some(gpu) = gpu() else { return };
    let hashes = crate::models::model_hash_count();
    let mut s = delivered(false);
    let live = |now| Frame {
        seekable: false,
        ..frame(now)
    };
    // Before the first frame only the presented look is prepared; its
    // texture may still be on the worker.
    s.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    assert_eq!(prepared(&s)[0], "red.model");
    assert!(
        prepared(&s).iter().all(|n| n.starts_with("red.")),
        "{:?}",
        prepared(&s)
    );
    let mut now = 0.;
    let mut first = None;
    for _ in 0..500 {
        now += 20.;
        let (pixels, _) = fixture::render(&gpu, &mut s, &live(now)).unwrap();
        if s.ready_work.is_some() {
            first = Some(pixels);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(s.error().is_none(), "{:?}", s.error());
    let first = first.expect("a live frame drew");
    // The first frame shows the presented look, prepared alone.
    assert_eq!(hue(&first), 0);
    assert_eq!(prepared(&s), ["red.model", "red.tex"]);
    assert!(s.asset_work(), "the other looks are still to prepare");
    // Later frames prepare the rest, the textures off this thread.
    for _ in 0..500 {
        if !s.asset_work() {
            break;
        }
        now += 20.;
        fixture::render(&gpu, &mut s, &live(now)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(prepared(&s).len(), 6, "{:?}", prepared(&s));
    assert!(s.sim().unwrap().device_assets_ready());
    // A generated model's digest is its identity: the renderer hashes none.
    assert_eq!(crate::models::model_hash_count(), hashes);
}

#[test]
fn the_agents_clock_prepares_every_look_before_its_first_frame() {
    let Some(gpu) = gpu() else { return };
    let mut s = delivered(true);
    let (pixels, _) = fixture::render(&gpu, &mut s, &frame(20.)).unwrap();
    assert_eq!(hue(&pixels), 0);
    assert_eq!(prepared(&s).len(), 6, "{:?}", prepared(&s));
    assert!(!s.asset_work());
}
