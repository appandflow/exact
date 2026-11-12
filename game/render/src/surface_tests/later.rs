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
/// Which look the die shows: a new world when it changes.
#[derive(Default, exact_game::Args)]
struct Shown {
    look: u32,
}
struct Looks;
impl Game for Looks {
    const ID: &'static str = "registered-looks";
    type Args = Shown;
    fn setup(w: &mut World, shown: &Shown) {
        for (i, (model, texture, _)) in LOOKS.into_iter().enumerate() {
            let mesh = w.generated_model(model, quad(texture)).unwrap();
            if i == shown.look as usize {
                w.spawn_named("die", (Transform::default(), mesh));
            }
        }
        w.spawn((Transform::at(0., 0., 4.), Camera::default()));
    }
    fn tick(_: &mut World, _: &Input, _: &Shown) {}
}
fn delivered(clock_seekable: bool) -> WorldSurface<Looks, crate::ModelExecutor, true> {
    let mut s = WorldSurface::<Looks, crate::ModelExecutor, true>::default();
    s.device_ready(exact_gpu::wgpu::Features::empty());
    s.bind(&[Value::Number(0u32.into())], None).unwrap();
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

fn live(now: f64) -> Frame {
    Frame {
        seekable: false,
        ..frame(now)
    }
}

#[test]
fn a_look_shown_before_the_first_frame_is_released_from_waiting() {
    let Some(gpu) = gpu() else { return };
    let mut s = delivered(false);
    s.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    // Green waited for the first frame; then a new world shows it first.
    s.bind(&[Value::Number(1u32.into())], None).unwrap();
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
    let first = first.expect("the first frame drew the look now shown");
    assert_eq!(hue(&first), 1);
    assert!(prepared(&s).contains(&"green.tex"), "{:?}", prepared(&s));
}

#[test]
fn the_agent_taking_the_clock_prepares_what_waited() {
    let Some(gpu) = gpu() else { return };
    let mut s = delivered(false);
    s.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    assert!(!prepared(&s).contains(&"blue.tex"));
    s.clock(true);
    let (pixels, _) = fixture::render(&gpu, &mut s, &frame(20.)).unwrap();
    assert_eq!(hue(&pixels), 0);
    // Nothing waits, and nothing is left on the worker, under the agent's clock.
    assert!(!s.asset_work());
    assert_eq!(prepared(&s).len(), 6, "{:?}", prepared(&s));
    assert!(s.sim().unwrap().device_assets_ready());
}
