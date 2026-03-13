use ash::vk;
use ash::{Device, Entry, Instance};

use super::{ApiConfig, ApiError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextState {
    Created,
    InstanceReady,
    DeviceReady,
}

pub struct VulkanHandles {
    pub entry: Option<Entry>,
    pub instance: Option<Instance>,
    pub physical_device: Option<vk::PhysicalDevice>,
    pub device: Option<Device>,
    pub graphics_queue: Option<vk::Queue>,
    pub present_queue: Option<vk::Queue>,
    pub transfer_queue: Option<vk::Queue>,
}

impl Default for VulkanHandles {
    fn default() -> Self {
        Self {
            entry: None,
            instance: None,
            physical_device: None,
            device: None,
            graphics_queue: None,
            present_queue: None,
            transfer_queue: None,
        }
    }
}

pub struct VulkanContext {
    config: ApiConfig,
    state: ContextState,
    handles: VulkanHandles,
    pub required_instance_extensions: Vec<String>,
    pub required_device_extensions: Vec<String>,
    pub validation_layers: Vec<String>,
}

impl VulkanContext {
    pub fn config(&self) -> &ApiConfig {
        &self.config
    }

    pub fn state(&self) -> ContextState {
        self.state
    }

    pub fn set_state(&mut self, state: ContextState) {
        self.state = state;
    }

    pub fn handles(&self) -> &VulkanHandles {
        &self.handles
    }

    pub fn handles_mut(&mut self) -> &mut VulkanHandles {
        &mut self.handles
    }
}

#[derive(Debug, Clone)]
pub struct VulkanContextBuilder {
    config: ApiConfig,
    required_instance_extensions: Vec<String>,
    required_device_extensions: Vec<String>,
    validation_layers: Vec<String>,
}

impl VulkanContextBuilder {
    pub fn new(config: ApiConfig) -> Self {
        Self {
            config,
            required_instance_extensions: vec![
                "VK_KHR_surface".to_string(),
                "VK_EXT_debug_utils".to_string(),
            ],
            required_device_extensions: vec!["VK_KHR_swapchain".to_string()],
            validation_layers: vec!["VK_LAYER_KHRONOS_validation".to_string()],
        }
    }

    pub fn config(mut self, config: ApiConfig) -> Self {
        self.config = config;
        self
    }

    pub fn require_instance_extension(mut self, extension: impl Into<String>) -> Self {
        self.required_instance_extensions.push(extension.into());
        self
    }

    pub fn require_device_extension(mut self, extension: impl Into<String>) -> Self {
        self.required_device_extensions.push(extension.into());
        self
    }

    pub fn validation_layer(mut self, layer: impl Into<String>) -> Self {
        self.validation_layers.push(layer.into());
        self
    }

    pub fn build_stub(self) -> Result<VulkanContext, ApiError> {
        if self.config.frames_in_flight == 0 {
            return Err(ApiError::InvalidConfig {
                reason: "frames_in_flight must be >= 1".to_string(),
            });
        }

        Ok(VulkanContext {
            config: self.config,
            state: ContextState::Created,
            handles: VulkanHandles::default(),
            required_instance_extensions: self.required_instance_extensions,
            required_device_extensions: self.required_device_extensions,
            validation_layers: self.validation_layers,
        })
    }
}
