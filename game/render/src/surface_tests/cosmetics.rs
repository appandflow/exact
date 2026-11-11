//! A look changed at any time (issues #88, #89, #90): a die's skin is a
//! texture named per instance, its shape a cosmetic model; each change keeps
//! the old look until the new one can draw, and releases what is no longer shown.
use super::*;
use exact_game::asset::{AlphaMode, MaterialData, MeshData, Model, Node, TextureData};

#[derive(Default, exact_game::Args)]
struct Look {
    /// 0: the model's own texture; otherwise the skin's atlas.
    #[live]
    skin: u32,
    /// 0: the square shape; 1: the round one; 2: one whose texture fails.
    #[live]
    shape: u32,
}
struct Dice;
impl Game for Dice {
    const ID: &'static str = "cosmetic-looks";
    type Args = Look;
    fn setup(w: &mut World, _: &Look) {
        w.spawn_named("die", (Transform::default(), Mesh::asset("square.model")));
        w.spawn((Transform::at(0., 0., 4.), Camera::default()));
    }
    fn tick(w: &mut World, _: &Input, look: &Look) {
        let shape = ["square.model", "round.model", "broken.model"][look.shape as usize];
        if !matches!(&*w.require::<Mesh>("die"), Mesh::Asset(name) if name == shape) {
            *w.require_mut::<Mesh>("die") = Mesh::asset(shape);
        }
    }
    fn present(p: &mut exact_game::Present<'_>, look: &Look) {
        let die = p.named("die").unwrap();
        let Mesh::Asset(shape) = (*p.require::<Mesh>(die)).clone() else {
            unreachable!()
        };
        let texture = match shape.as_str() {
            "round.model" => "yellow.tex",
            "broken.model" => "broken.tex",
            _ => "red.tex",
        };
        p.cosmetic_model(&shape, || quad(texture)).unwrap();
        if look.skin != 0 {
            let atlas = ["", "blue.tex", "green.tex"][look.skin as usize];
            p.insert(
                die,
                exact_game::MaterialOverrides(vec![exact_game::MaterialOverride {
                    material: 0,
                    textures: exact_game::TextureSlots {
                        base_color: Some(atlas.into()),
                        ..Default::default()
                    },
                    ..Default::default()
                }]),
            );
        }
    }
}
/// A square facing the camera, sampling `texture`.
fn quad(texture: &str) -> Model {
    let mesh = MeshData {
        positions: vec![-2., -2., 0., 2., -2., 0., 2., 2., 0., -2., 2., 0.],
        normals: [0., 0., 1.].repeat(4),
        uvs: vec![0., 1., 1., 1., 1., 0., 0., 0.],
        indices: vec![0, 1, 2, 0, 2, 3],
        bounds: [-2., -2., 0., 2., 2., 0.],
        ..Default::default()
    };
    Model {
        bounds: mesh.bounds,
        meshes: vec![mesh],
        materials: vec![MaterialData {
            base_color_texture: Some(0),
            metallic: 0.,
            alpha_mode: AlphaMode::Opaque,
            double_sided: true,
            ..Default::default()
        }],
        textures: vec![texture.into()],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    }
}
/// A solid 4×4 sRGB texture with its mips, as a delivered `.tex` file.
fn solid(rgb: [u8; 3]) -> Vec<u8> {
    let texel = [rgb[0], rgb[1], rgb[2], 255];
    exact_game::bin::to_vec(&TextureData {
        width: 4,
        height: 4,
        mips: [16, 4, 1].map(|n| texel.repeat(n)).to_vec(),
        srgb: true,
        ..Default::default()
    })
}
/// The centre pixel's dominant channel: 0 red, 1 green, 2 blue, 3 yellow.
fn hue(pixels: &fixture::Pixels) -> usize {
    let [r, g, b, _] = pixels.at(32, 32);
    let (r, g, b) = (u32::from(r), u32::from(g), u32::from(b));
    assert!(r.max(g).max(b) > 20, "the die draws: {r} {g} {b}");
    if r > 2 * b && g > 2 * b && r.abs_diff(g) * 4 < r {
        3
    } else if r > 2 * g.max(b) {
        0
    } else if g > 2 * r.max(b) {
        1
    } else {
        assert!(b > 2 * r.max(g), "{r} {g} {b}");
        2
    }
}

#[test]
fn a_skin_and_a_shape_change_keep_the_old_look_until_the_new_one_draws() {
    let Some(gpu) = gpu() else { return };
    let mut s = WorldSurface::<Dice, crate::ModelExecutor, true>::default();
    s.device_ready(exact_gpu::wgpu::Features::empty());
    let bind = |s: &mut WorldSurface<Dice, crate::ModelExecutor, true>, skin: u32, shape: u32| {
        s.bind(
            &[Value::Number(skin.into()), Value::Number(shape.into())],
            None,
        )
        .unwrap();
    };
    bind(&mut s, 0, 0);
    let mut now = 0.;
    let mut draw = |s: &mut WorldSurface<Dice, crate::ModelExecutor, true>| {
        now += 50.;
        hue(&fixture::render(&gpu, s, &frame(now)).unwrap().0)
    };
    let deliver = |s: &mut WorldSurface<Dice, crate::ModelExecutor, true>, name, rgb| {
        s.asset(name, Ok(&solid(rgb)));
    };
    // The cosmetic model is never fetched: only its texture is.
    assert_eq!(s.assets().requests, ["red.tex"]);
    deliver(&mut s, "red.tex", [255, 0, 0]);
    assert_eq!(draw(&mut s), 0);
    assert!(s.sim().unwrap().world().model("square.model").is_none());

    // A skin whose atlas is not resident: the old look stays until it lands.
    bind(&mut s, 1, 0);
    assert_eq!(draw(&mut s), 0);
    assert_eq!(s.assets().requests, ["blue.tex"]);
    assert_eq!(draw(&mut s), 0);
    deliver(&mut s, "blue.tex", [0, 0, 255]);
    assert_eq!(draw(&mut s), 2);
    assert!(
        s.assets().retired.is_empty(),
        "the model keeps its own texture"
    );

    // Another skin: the drawn atlas is held until the next one draws, then released.
    bind(&mut s, 2, 0);
    assert_eq!(draw(&mut s), 2);
    let changes = s.assets();
    assert_eq!(changes.requests, ["green.tex"]);
    assert!(changes.retired.is_empty(), "{:?}", changes.retired);
    deliver(&mut s, "green.tex", [0, 255, 0]);
    assert_eq!(draw(&mut s), 1);
    let retired = s.assets().retired;
    assert!(retired.iter().any(|n| n.contains("blue")), "{retired:?}");
    assert!(!s.render.as_ref().unwrap().0.models.textures["blue.tex"].active);

    // Back to the model's own texture, resident: it draws at once, and nothing
    // shows the skin any more.
    bind(&mut s, 0, 0);
    assert_eq!(draw(&mut s), 0);
    let retired = s.assets().retired;
    assert!(retired.iter().any(|n| n.contains("green")), "{retired:?}");

    // A shape whose texture fails: the die keeps drawing the square, which
    // stays resident.
    bind(&mut s, 0, 2);
    assert_eq!(draw(&mut s), 0);
    assert_eq!(s.assets().requests, ["broken.tex"]);
    s.asset("broken.tex", Err(AssetError::Failed("gone".into())));
    assert_eq!(draw(&mut s), 0);
    assert!(s.assets().retired.is_empty());
    assert_eq!(draw(&mut s), 0);

    // A shape still in flight: the die keeps drawing the square.
    bind(&mut s, 0, 1);
    assert_eq!(draw(&mut s), 0);
    let changes = s.assets();
    assert_eq!(changes.requests, ["yellow.tex"]);
    // Nothing shows the broken shape any more; the square stands in, held.
    let mut retired = changes.retired;
    retired.sort();
    assert_eq!(retired, ["broken.model", "broken.tex"]);
    assert_eq!(draw(&mut s), 0);
    deliver(&mut s, "yellow.tex", [255, 255, 0]);
    assert_eq!(draw(&mut s), 3);
    // Nothing shows the square any more: it and its texture retire.
    let names = s.assets().retired.join(" ");
    for name in ["square.model", "red"] {
        assert!(names.contains(name), "{name}: {names}");
    }
    // No draw uses a skin's material any more: it was reclaimed.
    assert!(s.render.as_ref().unwrap().0.models.variants.is_empty());
    let sim = s.sim().unwrap();
    assert_eq!(
        sim.presentation_models()
            .map(|(n, _)| n)
            .collect::<Vec<_>>(),
        ["round.model"]
    );
}
