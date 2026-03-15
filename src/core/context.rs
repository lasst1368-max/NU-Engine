use ash::vk;
use ash::{Device, Entry, Instance};
use std::ffi::CString;

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

    pub fn build_headless(self) -> Result<VulkanContext, ApiError> {
        if self.config.frames_in_flight == 0 {
            return Err(ApiError::InvalidConfig {
                reason: "frames_in_flight must be >= 1".to_string(),
            });
        }

        let entry = unsafe {
            Entry::load().map_err(|err| ApiError::Window {
                reason: format!("failed to load Vulkan entry: {err}"),
            })?
        };
        let instance = create_headless_instance(
            &entry,
            &self.config,
            &self.validation_layers,
            &self.required_instance_extensions,
        )?;
        let (physical_device, queue_family_index) = pick_headless_physical_device(&instance)?;
        let (device, graphics_queue) = create_headless_logical_device(
            &instance,
            physical_device,
            queue_family_index,
            &self.required_device_extensions,
        )?;

        Ok(VulkanContext {
            config: self.config,
            state: ContextState::DeviceReady,
            handles: VulkanHandles {
                entry: Some(entry),
                instance: Some(instance),
                physical_device: Some(physical_device),
                device: Some(device.clone()),
                graphics_queue: Some(graphics_queue),
                present_queue: Some(graphics_queue),
                transfer_queue: Some(graphics_queue),
            },
            required_instance_extensions: self.required_instance_extensions,
            required_device_extensions: self.required_device_extensions,
            validation_layers: self.validation_layers,
        })
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            if let Some(device) = self.handles.device.take() {
                let _ = device.device_wait_idle();
                device.destroy_device(None);
            }
            if let Some(instance) = self.handles.instance.take() {
                instance.destroy_instance(None);
            }
        }
        self.handles.entry = None;
        self.handles.graphics_queue = None;
        self.handles.present_queue = None;
        self.handles.transfer_queue = None;
        self.handles.physical_device = None;
    }
}

fn create_headless_instance(
    entry: &Entry,
    config: &ApiConfig,
    validation_layers: &[String],
    required_instance_extensions: &[String],
) -> Result<Instance, ApiError> {
    let application_name =
        CString::new(config.application_name.as_str()).map_err(|_| ApiError::InvalidConfig {
            reason: "application_name cannot contain nul characters".to_string(),
        })?;
    let engine_name =
        CString::new(config.engine_name.as_str()).map_err(|_| ApiError::InvalidConfig {
            reason: "engine_name cannot contain nul characters".to_string(),
        })?;
    let app_info = vk::ApplicationInfo::default()
        .application_name(application_name.as_c_str())
        .application_version(vk::make_api_version(0, 0, 0, 1))
        .engine_name(engine_name.as_c_str())
        .engine_version(vk::make_api_version(0, 0, 0, 1))
        .api_version(vk::API_VERSION_1_3);

    let extension_cstrings: Vec<CString> = required_instance_extensions
        .iter()
        .map(|name| {
            CString::new(name.as_str()).map_err(|_| ApiError::InvalidConfig {
                reason: format!("instance extension contains nul characters: {name}"),
            })
        })
        .collect::<Result<_, _>>()?;
    let extension_ptrs: Vec<_> = extension_cstrings
        .iter()
        .map(|name| name.as_ptr())
        .collect();

    let layer_cstrings: Vec<CString> = if config.enable_validation {
        validation_layers
            .iter()
            .map(|name| {
                CString::new(name.as_str()).map_err(|_| ApiError::InvalidConfig {
                    reason: format!("validation layer contains nul characters: {name}"),
                })
            })
            .collect::<Result<_, _>>()?
    } else {
        Vec::new()
    };
    let layer_ptrs: Vec<_> = layer_cstrings.iter().map(|layer| layer.as_ptr()).collect();

    let info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&extension_ptrs)
        .enabled_layer_names(&layer_ptrs);

    vk_result(
        unsafe { entry.create_instance(&info, None) },
        "create_instance(headless)",
    )
}

fn pick_headless_physical_device(
    instance: &Instance,
) -> Result<(vk::PhysicalDevice, u32), ApiError> {
    let physical_devices = vk_result(
        unsafe { instance.enumerate_physical_devices() },
        "enumerate_physical_devices(headless)",
    )?;
    for physical_device in physical_devices {
        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        for (index, family) in queue_family_properties.iter().enumerate() {
            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                return Ok((physical_device, index as u32));
            }
        }
    }
    Err(ApiError::InvalidConfig {
        reason: "no physical device with graphics queue support found".to_string(),
    })
}

fn create_headless_logical_device(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    required_device_extensions: &[String],
) -> Result<(Device, vk::Queue), ApiError> {
    let queue_priorities = [1.0_f32];
    let queue_info = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities)];
    let extension_cstrings: Vec<CString> = required_device_extensions
        .iter()
        .map(|name| {
            CString::new(name.as_str()).map_err(|_| ApiError::InvalidConfig {
                reason: format!("device extension contains nul characters: {name}"),
            })
        })
        .collect::<Result<_, _>>()?;
    let extension_ptrs: Vec<_> = extension_cstrings
        .iter()
        .map(|name| name.as_ptr())
        .collect();
    let info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_info)
        .enabled_extension_names(&extension_ptrs);
    let device = vk_result(
        unsafe { instance.create_device(physical_device, &info, None) },
        "create_device(headless)",
    )?;
    let graphics_queue = unsafe { device.get_device_queue(queue_family_index, 0) };
    Ok((device, graphics_queue))
}

fn vk_result<T>(result: Result<T, vk::Result>, context: &'static str) -> Result<T, ApiError> {
    result.map_err(|result| ApiError::Vulkan { context, result })
}
