use ash::vk;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ApiError {
    InvalidConfig {
        reason: String,
    },
    InvalidFrameState {
        reason: String,
    },
    ResourceNotFound {
        resource_type: &'static str,
        name: String,
    },
    Vulkan {
        context: &'static str,
        result: vk::Result,
    },
    Window {
        reason: String,
    },
    MissingImplementation(&'static str),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig { reason } => write!(f, "invalid API configuration: {reason}"),
            Self::InvalidFrameState { reason } => write!(f, "invalid frame state: {reason}"),
            Self::ResourceNotFound {
                resource_type,
                name,
            } => write!(f, "resource not found ({resource_type}): {name}"),
            Self::Vulkan { context, result } => {
                write!(f, "vulkan error during {context}: {result:?}")
            }
            Self::Window { reason } => write!(f, "window error: {reason}"),
            Self::MissingImplementation(detail) => {
                write!(f, "missing implementation in boilerplate: {detail}")
            }
        }
    }
}

impl Error for ApiError {}
