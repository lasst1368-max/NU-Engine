pub const MAX_POINT_LIGHTS: usize = 4;
pub const MAX_SPOT_LIGHTS: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            position: [1.8, 2.6, -2.4],
            color: [1.0, 0.97, 0.92],
            intensity: 4.5,
            range: 12.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: [-0.55, 0.85, -0.35],
            color: [0.42, 0.46, 0.54],
            intensity: 0.42,
        }
    }
}

/// A spotlight with inner/outer cone angles. Attenuation is 1/(1 + d²/range²).
/// The cone blend is smooth: full intensity inside `inner_cone_degrees`, zero outside
/// `outer_cone_degrees`, with a smooth cosine falloff between the two angles.
#[derive(Debug, Clone, Copy)]
pub struct SpotLight {
    pub position: [f32; 3],
    /// Unit direction vector the spotlight points toward.
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    /// Angle (degrees) of the fully-lit inner cone.
    pub inner_cone_degrees: f32,
    /// Angle (degrees) of the outer cutoff cone.
    pub outer_cone_degrees: f32,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            position: [0.0, 5.0, 0.0],
            direction: [0.0, -1.0, 0.0],
            color: [1.0, 0.95, 0.88],
            intensity: 3.0,
            range: 15.0,
            inner_cone_degrees: 20.0,
            outer_cone_degrees: 35.0,
        }
    }
}

impl SpotLight {
    /// Returns the cosine of the outer cone angle, used for GPU-side cutoff tests.
    pub fn outer_cos(&self) -> f32 {
        self.outer_cone_degrees.to_radians().cos()
    }

    /// Returns the cosine of the inner cone angle, used for GPU-side smooth blend.
    pub fn inner_cos(&self) -> f32 {
        self.inner_cone_degrees.to_radians().cos()
    }

    /// Returns `[cos_inner - cos_outer]` — the width of the penumbra band.
    /// A larger value = softer edge; zero = hard cutoff.
    pub fn cone_penumbra_width(&self) -> f32 {
        (self.inner_cos() - self.outer_cos()).max(0.0001)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowMode {
    Off,
    Live,
}

#[derive(Debug, Clone, Copy)]
pub struct LiveShadowConfig {
    pub max_distance: f32,
    pub filter_radius: f32,
}

impl Default for LiveShadowConfig {
    fn default() -> Self {
        Self {
            max_distance: 18.0,
            filter_radius: 1.25,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShadowConfig {
    pub mode: ShadowMode,
    pub minimum_visibility: f32,
    pub bias: f32,
    pub live: LiveShadowConfig,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            mode: ShadowMode::Live,
            minimum_visibility: 0.18,
            bias: 0.004,
            live: LiveShadowConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LightingConfig {
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub point_lights: [PointLight; MAX_POINT_LIGHTS],
    pub point_light_shadow_flags: [bool; MAX_POINT_LIGHTS],
    pub point_light_count: usize,
    pub spot_lights: [SpotLight; MAX_SPOT_LIGHTS],
    pub spot_light_count: usize,
    pub fill_light: DirectionalLight,
    pub shadows: ShadowConfig,
    pub specular_strength: f32,
    pub shininess: f32,
}

impl Default for LightingConfig {
    fn default() -> Self {
        let default_point_light = PointLight::default();
        Self {
            ambient_color: [0.16, 0.18, 0.22],
            ambient_intensity: 0.55,
            point_lights: [default_point_light; MAX_POINT_LIGHTS],
            point_light_shadow_flags: [true, false, false, false],
            point_light_count: 1,
            spot_lights: [SpotLight::default(); MAX_SPOT_LIGHTS],
            spot_light_count: 0,
            fill_light: DirectionalLight::default(),
            shadows: ShadowConfig::default(),
            specular_strength: 0.42,
            shininess: 48.0,
        }
    }
}

impl LightingConfig {
    pub fn clear_point_lights(&mut self) {
        self.point_light_count = 0;
        self.point_light_shadow_flags = [false; MAX_POINT_LIGHTS];
    }

    pub fn set_single_point_light(&mut self, light: PointLight, casts_shadow: bool) {
        self.clear_point_lights();
        self.point_lights[0] = light;
        self.point_light_shadow_flags[0] = casts_shadow;
        self.point_light_count = 1;
    }

    pub fn push_point_light(&mut self, light: PointLight, casts_shadow: bool) -> bool {
        if self.point_light_count >= MAX_POINT_LIGHTS {
            return false;
        }
        let index = self.point_light_count;
        self.point_lights[index] = light;
        self.point_light_shadow_flags[index] = casts_shadow;
        self.point_light_count += 1;
        true
    }

    pub fn clear_spot_lights(&mut self) {
        self.spot_light_count = 0;
    }

    /// Appends a spotlight, returning `false` when the `MAX_SPOT_LIGHTS` limit is reached.
    pub fn push_spot_light(&mut self, light: SpotLight) -> bool {
        if self.spot_light_count >= MAX_SPOT_LIGHTS {
            return false;
        }
        self.spot_lights[self.spot_light_count] = light;
        self.spot_light_count += 1;
        true
    }

    pub fn set_single_spot_light(&mut self, light: SpotLight) {
        self.clear_spot_lights();
        self.spot_lights[0] = light;
        self.spot_light_count = 1;
    }
}
