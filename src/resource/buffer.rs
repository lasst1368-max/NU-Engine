#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferUsage {
    Vertex,
    Index,
    Uniform,
    Storage,
    Staging,
}

#[derive(Debug, Clone)]
pub struct BufferDesc {
    pub name: String,
    pub size: u64,
    pub usage: BufferUsage,
    pub host_visible: bool,
}
