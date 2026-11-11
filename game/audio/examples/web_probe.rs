//! Size probe: cargo build -p exact-game-audio --example web_probe --profile web --target wasm32-unknown-unknown
#[cfg(target_arch = "wasm32")]
mod probe {
    use exact_game::{audio::Synth, World};
    use exact_game_audio::{Output, Pcm, Player, Transport, WebOutput};
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub struct Probe {
        player: Player<WebOutput>,
        world: World,
        transport: Transport,
    }
    #[wasm_bindgen]
    impl Probe {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Result<Probe, JsValue> {
            let mut world = World::new(60, 0);
            world.sounds([("chime", Synth::sine(880.0))]);
            world.play("chime").ui().start();
            Ok(Self {
                player: Player::new(WebOutput::new()?, 48000),
                world,
                transport: Transport::default(),
            })
        }
        pub fn unlock(&mut self) {
            self.player.output.unlock();
        }
        pub fn ready(&self) -> bool {
            self.player.output.ready()
        }
        pub fn frame(&mut self) {
            self.player.sync(&self.world, None, self.transport);
        }
        pub fn start(&mut self, samples: Vec<f32>, rate: u32) -> bool {
            let pcm = Pcm::F32(samples.into());
            self.player.output.start(1, &pcm, rate, false, 0, 1.0)
        }
        pub fn set(&mut self, left: f32, right: f32) {
            self.player.output.set(1, left, right);
        }
    }
}
