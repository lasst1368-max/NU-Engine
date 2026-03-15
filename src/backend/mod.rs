#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum GraphicsBackendKind {
    Vulkan = 1,
    Dx12 = 2,
    Metal = 3,
}

impl GraphicsBackendKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Dx12 => "dx12",
            Self::Metal => "metal",
        }
    }

    pub const fn dll_name(self) -> &'static str {
        match self {
            Self::Vulkan => "nu-vlk.dll",
            Self::Dx12 => "nu-Dx12.dll",
            Self::Metal => "libnu-metal.dylib",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendInfo {
    pub kind: GraphicsBackendKind,
    pub crate_name: &'static str,
    pub dll_name: &'static str,
    pub display_name: &'static str,
}

pub const VULKAN_BACKEND_INFO: BackendInfo = BackendInfo {
    kind: GraphicsBackendKind::Vulkan,
    crate_name: "nu",
    dll_name: "nu-vlk.dll",
    display_name: "NU Vulkan",
};

pub const DX12_BACKEND_INFO: BackendInfo = BackendInfo {
    kind: GraphicsBackendKind::Dx12,
    crate_name: "nu-dx12",
    dll_name: "nu-Dx12.dll",
    display_name: "NU Direct3D 12",
};

pub const METAL_BACKEND_INFO: BackendInfo = BackendInfo {
    kind: GraphicsBackendKind::Metal,
    crate_name: "nu-metal",
    dll_name: "libnu-metal.dylib",
    display_name: "NU Metal",
};

pub const ALL_BACKEND_INFO: [BackendInfo; 3] =
    [VULKAN_BACKEND_INFO, DX12_BACKEND_INFO, METAL_BACKEND_INFO];

pub const fn backend_info(kind: GraphicsBackendKind) -> BackendInfo {
    match kind {
        GraphicsBackendKind::Vulkan => VULKAN_BACKEND_INFO,
        GraphicsBackendKind::Dx12 => DX12_BACKEND_INFO,
        GraphicsBackendKind::Metal => METAL_BACKEND_INFO,
    }
}

pub const fn backend_count() -> usize {
    ALL_BACKEND_INFO.len()
}
