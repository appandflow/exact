//! Local and directional lights on the GPU: spot cones, more than sixteen
//! lights, the second directional light and photometric units.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Gpu, Surface, Value};

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(fixture::device())
}

#[derive(Default, Args)]
struct Scene {
    kind: u32,
}
const SPOT: u32 = 0;
const POINT: u32 = 1;
const MANY: u32 = 2;
const FILL: u32 = 3;
const SPOT_SHADOW: u32 = 4;
const SPOT_UNSHADOWED: u32 = 5;
const POINT_SHADOW: u32 = 6;
const POINT_UNSHADOWED: u32 = 7;
// A top-down orthographic view: 20 m tall, so 18 px per metre at 640 × 360,
// with +X right and +Z down the screen.
const PX: f32 = 18.;
struct Lights;
impl Game for Lights {
    const ID: &'static str = "render-lights";
    type Args = Scene;
    fn setup(w: &mut World, args: &Scene) {
        w.insert_resource(Environment {
            ambient: 0.,
            bloom: None,
            fog: None,
            background: Some([0.; 3].into()),
            ..Default::default()
        });
        w.spawn((
            Transform::default(),
            Mesh::plane(40., 40.),
            Material::rgb(0.6, 0.6, 0.6),
        ));
        w.spawn((
            Transform::at(0., 10., 0.).looking_at(Vec3::ZERO, -Vec3::Z),
            Camera::orthographic(20.),
        ));
        let down = |x: f32, y: f32, z: f32| {
            Transform::at(x, y, z).looking_at(Vec3::new(x, 0., z), Vec3::Z)
        };
        let sun = DirectionalLight {
            illuminance: 0.,
            ..Default::default()
        };
        w.spawn((down(0., 5., 0.), sun));
        match args.kind {
            SPOT => {
                w.spawn((
                    down(0., 4., 0.),
                    SpotLight {
                        intensity: 20_000.,
                        range: 12.,
                        inner: 0.2,
                        outer: 0.3,
                        ..Default::default()
                    },
                ));
            }
            POINT => {
                w.spawn((
                    down(0., 4., 0.),
                    PointLight {
                        intensity: 20_000.,
                        range: 12.,
                        ..Default::default()
                    },
                ));
            }
            MANY => {
                for i in 0..40 {
                    let (x, z) = ((i % 8) as f32 * 2. - 7., (i / 8) as f32 * 2. - 4.);
                    w.spawn((
                        Transform::at(x, 0.5, z),
                        PointLight {
                            intensity: 2_000.,
                            range: 0.9,
                            ..Default::default()
                        },
                    ));
                }
            }
            SPOT_SHADOW | SPOT_UNSHADOWED => {
                let spot = w.spawn((
                    down(0., 4., 0.),
                    SpotLight {
                        intensity: 20_000.,
                        range: 12.,
                        inner: 0.2,
                        outer: 0.3,
                        ..Default::default()
                    },
                ));
                if args.kind == SPOT_SHADOW {
                    w.insert(spot, LightShadows);
                }
                // Halfway down: its shadow on the floor is twice its width.
                w.spawn((
                    Transform {
                        position: Vec3::new(0., 2., 0.),
                        scale: Vec3::new(0.6, 0.1, 0.6),
                        ..Default::default()
                    },
                    Mesh::cube(1.),
                    Material::rgb(0.6, 0.6, 0.6),
                ));
            }
            POINT_SHADOW | POINT_UNSHADOWED => {
                let lamp = w.spawn((
                    Transform::at(0., 2., 0.),
                    PointLight {
                        intensity: 20_000.,
                        range: 8.,
                        ..Default::default()
                    },
                ));
                if args.kind == POINT_SHADOW {
                    w.insert(lamp, LightShadows);
                }
                // Walls on +X and +Z: two cube faces' maps.
                for (x, z, size) in [
                    (2., 0., Vec3::new(0.2, 2., 3.)),
                    (0., 2.5, Vec3::new(3., 2., 0.2)),
                ] {
                    w.spawn((
                        Transform {
                            position: Vec3::new(x, 1., z),
                            scale: size,
                            ..Default::default()
                        },
                        Mesh::cube(1.),
                        Material::rgb(0.6, 0.6, 0.6),
                    ));
                }
            }
            _ => {
                w.spawn((
                    down(0., 5., 0.),
                    DirectionalLight {
                        illuminance: 10_000.,
                        shadows: true,
                        ..Default::default()
                    },
                ));
            }
        }
    }
    fn tick(_: &mut World, _: &Input, _: &Scene) {}
}

fn render(gpu: &Gpu, kind: u32, name: &str) -> (fixture::Pixels, String) {
    let mut surface = WorldSurface::<Lights>::default();
    surface
        .bind(&[Value::Number(f64::from(kind))], None)
        .unwrap();
    let frame = Frame {
        width: 640.,
        height: 360.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    let (pixels, _) = fixture::render(gpu, &mut surface, &frame).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    pixels.save(name);
    let state = surface.agent(r#"{"op":"state"}"#).unwrap();
    (pixels, state)
}
// Mean red channel over a 5 × 5 patch at world (x, z) on the floor.
fn at(image: &fixture::Pixels, x: f32, z: f32) -> f64 {
    let (cx, cy) = ((320. + x * PX) as u32, (180. + z * PX) as u32);
    let mut sum = 0.;
    for y in cy - 2..=cy + 2 {
        for x in cx - 2..=cx + 2 {
            sum += f64::from(image.at(x, y)[0]);
        }
    }
    sum / 25.
}

#[test]
fn a_spot_light_lights_its_cone_and_a_point_light_everywhere_in_range() {
    let Some(gpu) = gpu() else { return };
    let (spot, _) = render(&gpu, SPOT, "lights-spot");
    let (point, _) = render(&gpu, POINT, "lights-point");
    // The cone's edge on the floor is 4 m × tan 0.3 ≈ 1.24 m from its axis.
    let values = [
        at(&spot, 0., 0.),
        at(&spot, 0.6, 0.),
        at(&spot, 2.5, 0.),
        at(&point, 2.5, 0.),
    ];
    eprintln!("spot axis, inside, outside; point outside the cone: {values:?}");
    assert!(values[0] > 60. && values[1] > 60., "{values:?}");
    assert_eq!(values[2], 0., "{values:?}");
    assert!(values[3] > 30., "{values:?}");
    // Inside the inner cone a spot equals the point light of the same intensity.
    assert!((values[0] - at(&point, 0., 0.)).abs() <= 1., "{values:?}");
}

#[test]
fn forty_point_lights_all_light_the_floor_and_none_are_dropped() {
    let Some(gpu) = gpu() else { return };
    let (image, state) = render(&gpu, MANY, "lights-forty");
    let mut dark = Vec::new();
    for i in 0..40 {
        let (x, z) = ((i % 8) as f32 * 2. - 7., (i / 8) as f32 * 2. - 4.);
        if at(&image, x, z) < 40. {
            dark.push(i);
        }
        // Between the lights the floor is out of every range.
        assert_eq!(at(&image, x + 1., z + 1.), 0., "light {i}");
    }
    assert!(dark.is_empty(), "unlit under lights {dark:?}");
    assert!(
        state.contains(r#""lights":{"drawn":40,"dropped":0}"#),
        "{state}"
    );
}

#[test]
fn the_second_directional_light_is_an_unshadowed_fill() {
    let Some(gpu) = gpu() else { return };
    let (image, _) = render(&gpu, FILL, "lights-fill");
    // The first light (the sun) is dark; the second lights the floor alone.
    assert!(at(&image, 0., 0.) > 100., "{}", at(&image, 0., 0.));
}

#[test]
fn a_shadowed_spot_light_darkens_the_floor_behind_a_caster() {
    let Some(gpu) = gpu() else { return };
    let (shadowed, _) = render(&gpu, SPOT_SHADOW, "lights-spot-shadow");
    let (open, _) = render(&gpu, SPOT_UNSHADOWED, "lights-spot-unshadowed");
    // 0.45 m off axis: outside the 0.6 m caster, inside its 1.2 m shadow.
    let values = [
        at(&shadowed, 0.45, 0.),
        at(&open, 0.45, 0.),
        at(&shadowed, 0.9, 0.),
    ];
    eprintln!("shadowed, unshadowed, beyond the shadow: {values:?}");
    assert!(values[0] < 5., "{values:?}");
    assert!(values[1] > 60., "{values:?}");
    assert!(values[2] > 40., "{values:?}");
}

#[test]
fn a_shadowed_point_light_casts_through_each_cube_face() {
    let Some(gpu) = gpu() else { return };
    let (shadowed, _) = render(&gpu, POINT_SHADOW, "lights-point-shadow");
    let (open, _) = render(&gpu, POINT_UNSHADOWED, "lights-point-unshadowed");
    // Behind the +X and +Z walls, and the open floor on -X and -Z.
    let behind = [at(&shadowed, 3., 0.), at(&shadowed, 0., 3.5)];
    let unshadowed = [at(&open, 3., 0.), at(&open, 0., 3.5)];
    let away = [at(&shadowed, -3., 0.), at(&shadowed, 0., -3.5)];
    eprintln!("behind {behind:?}; unshadowed {unshadowed:?}; away {away:?}");
    for i in 0..2 {
        assert!(behind[i] < 5., "{behind:?}");
        assert!(unshadowed[i] > 20., "{unshadowed:?}");
        assert!(
            (away[i] - unshadowed[i]).abs() <= 2.,
            "{away:?} {unshadowed:?}"
        );
    }
}

#[test]
fn toggling_a_shadowed_flashlight_reuses_its_shadow_layers() {
    use exact_game_render::{shapes, Batch, FrameInput, LightInput, Renderer};
    let Some(gpu) = gpu() else { return };
    let format = exact_gpu::wgpu::TextureFormat::Rgba8Unorm;
    let mut r = Renderer::new(&gpu.device, &gpu.queue, format);
    let (v, i) = shapes::cube();
    let cube = r.add_mesh(&v, &i);
    r.write_transforms_both(0, &[0., 0., 0., 0., 0., 0., 1., 1., 1., 1.])
        .unwrap();
    r.write_materials(0, &[0.5, 0.5, 0.5, 1., 0., 1., 0., 0., 0., 1., 1., 1.])
        .unwrap();
    r.set_batches(&[Batch::new(cube, 0..1)], &[0]).unwrap();
    let texture = gpu
        .device
        .create_texture(&exact_gpu::wgpu::TextureDescriptor {
            label: None,
            size: exact_gpu::wgpu::Extent3d {
                width: 32,
                height: 32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: exact_gpu::wgpu::TextureDimension::D2,
            format,
            usage: exact_gpu::wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
    let view = texture.create_view(&Default::default());
    let mut creations = Vec::new();
    for intensity in [3., 0., 3., 0., 3.] {
        let lights = [LightInput {
            position: Vec3::new(0., 3., 0.),
            color: Vec3::ONE,
            intensity,
            range: 10.,
            direction: -Vec3::Y,
            cone: Some([0.9, 0.8]),
            shadows: true,
        }];
        let frame = FrameInput {
            view: glam::camera::rh::view::look_at_mat4(Vec3::new(0., 2., 4.), Vec3::ZERO, Vec3::Y),
            lights: &lights,
            sun: None,
            ..Default::default()
        };
        creations.push(r.draw(&view, (32, 32), &frame).texture_creations);
    }
    assert!(creations.windows(2).all(|w| w[0] == w[1]), "{creations:?}");
}

#[derive(Default, Args)]
struct FloorView {
    model: bool,
    orthographic: bool,
    roughness: f32,
    camera_scale: f32,
}
struct Floor;
impl Game for Floor {
    const ID: &'static str = "floor-view-direction";
    const ASSETS: &'static [&'static str] = &["floor.model"];
    type Args = FloorView;
    fn setup(w: &mut World, args: &FloorView) {
        w.insert_resource(Environment {
            zenith: [0.2; 3],
            horizon: [0.2; 3],
            ground: [0.2; 3],
            bloom: None,
            fog: None,
            tone_mapping: ToneMapping::None,
            ..Default::default()
        });
        let floor = w.spawn((
            Transform::default(),
            if args.model {
                Mesh::asset("floor.model")
            } else {
                Mesh::plane(400., 400.)
            },
        ));
        if !args.model {
            w.insert(
                floor,
                Material {
                    roughness: args.roughness,
                    ..Material::rgb(0.5, 0.5, 0.5)
                },
            );
        }
        w.spawn((
            Transform::at(0., 6., 0.)
                .looking_at(Vec3::ZERO, -Vec3::Z)
                .with_scale([1., 1., args.camera_scale]),
            if args.orthographic {
                Camera::orthographic(10.)
            } else {
                Camera {
                    fov_y_degrees: 80.,
                    ..Default::default()
                }
            },
        ));
        w.spawn((
            Transform::at(1., 1., 0.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                shadows: false,
                ..Default::default()
            },
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &FloorView) {}
}

fn floor_shading(model: bool) {
    use exact_game::asset::{MaterialData, MeshData, Model, Node};
    let Some(gpu) = gpu() else { return };
    let (vertices, indices) = exact_game_render::shapes::plane();
    let bounds = [-200., 0., -200., 200., 0., 200.];
    let mut floor = Model {
        meshes: vec![MeshData {
            positions: vertices
                .iter()
                .flat_map(|v| v.position.map(|x| x * 400.))
                .collect(),
            normals: vertices.iter().flat_map(|v| v.normal).collect(),
            uvs: vertices.iter().flat_map(|v| v.uv).collect(),
            indices,
            bounds,
            ..Default::default()
        }],
        materials: vec![MaterialData {
            base_color: [0.5, 0.5, 0.5, 1.],
            metallic: 0.,
            ..Default::default()
        }],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        bounds,
        ..Default::default()
    };
    let frame = Frame {
        width: 160.,
        height: 100.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.,
    };
    for (orthographic, roughness) in [
        (true, 0.3),
        (true, 0.5),
        (true, 0.8),
        (true, 1.),
        (false, 0.3),
    ] {
        floor.materials[0].roughness = roughness;
        let scales: &[f32] = if orthographic { &[1., 0.5, 2.] } else { &[1.] };
        let mut unscaled = [0; 3];
        for &camera_scale in scales {
            let mut surface =
                WorldSurface::<Floor, exact_game_render::ModelExecutor, true>::default();
            surface.device_ready(exact_gpu::wgpu::Features::empty());
            surface
                .bind(
                    &FloorView {
                        model,
                        orthographic,
                        roughness,
                        camera_scale,
                    }
                    .values(),
                    None,
                )
                .unwrap();
            assert_eq!(surface.assets().requests, ["floor.model"]);
            surface.asset("floor.model", Ok(&exact_game::asset::encode_model(&floor)));
            assert!(
                !surface.sim().unwrap().is_loading(),
                "{}",
                surface.agent(r#"{"op":"state"}"#).unwrap()
            );
            let (pixels, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
            assert!(surface.error().is_none(), "{:?}", surface.error());
            // The orthographic row samples near x = -6, 0 and +6 metres, clear of edges.
            let row = [20, 80, 140].map(|x| pixels.at(x, 50)[0]);
            let spread = row.iter().max().unwrap() - row.iter().min().unwrap();
            eprintln!(
                "model={model}, orthographic={orthographic}, roughness={roughness}, \
                 camera_scale={camera_scale}: {row:?}"
            );
            assert!(
                row[1] > 20 && row[1] < 240,
                "the lit floor must be visible: {row:?}"
            );
            if orthographic {
                assert!(spread <= 1, "parallel view rays must shade alike: {row:?}");
                if camera_scale == 1. {
                    unscaled = row;
                } else {
                    assert!(
                        row.iter().zip(unscaled).all(|(a, b)| a.abs_diff(b) <= 1),
                        "camera scale must preserve lighting: {unscaled:?} -> {row:?}"
                    );
                }
            } else {
                assert!(
                    spread > 20,
                    "a perspective eye keeps its highlight: {row:?}"
                );
            }
        }
    }
}

#[test]
fn orthographic_primitive_shading_uses_parallel_view_rays() {
    floor_shading(false);
}

#[test]
fn orthographic_model_shading_uses_parallel_view_rays() {
    floor_shading(true);
}
