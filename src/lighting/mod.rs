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

#[derive(Debug, Clone, Copy)]
pub struct ShadowConfig {
    pub minimum_visibility: f32,
    pub bias: f32,
    pub point_light_radius: f32,
    pub point_samples: u32,
    pub directional_spread: f32,
    pub directional_samples: u32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            minimum_visibility: 0.18,
            bias: 0.02,
            point_light_radius: 0.16,
            point_samples: 5,
            directional_spread: 0.08,
            directional_samples: 3,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LightingConfig {
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub point_light: PointLight,
    pub fill_light: DirectionalLight,
    pub shadows: ShadowConfig,
    pub specular_strength: f32,
    pub shininess: f32,
}

impl Default for LightingConfig {
    fn default() -> Self {
        Self {
            ambient_color: [0.16, 0.18, 0.22],
            ambient_intensity: 0.55,
            point_light: PointLight::default(),
            fill_light: DirectionalLight::default(),
            shadows: ShadowConfig::default(),
            specular_strength: 0.42,
            shininess: 48.0,
        }
    }
}
