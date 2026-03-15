use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventListenerHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceEventKind {
    Texture,
    Mesh,
    Shader,
    Material,
    Scene,
    Font,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventCategoryMask(u32);

impl EventCategoryMask {
    pub const NONE: Self = Self(0);
    pub const APPLICATION: Self = Self(1 << 0);
    pub const INPUT: Self = Self(1 << 1);
    pub const WINDOW: Self = Self(1 << 2);
    pub const PHYSICS: Self = Self(1 << 3);
    pub const RESOURCE: Self = Self(1 << 4);
    pub const SCENE: Self = Self(1 << 5);
    pub const EDITOR: Self = Self(1 << 6);

    pub const ALL: Self = Self(
        Self::APPLICATION.0
            | Self::INPUT.0
            | Self::WINDOW.0
            | Self::PHYSICS.0
            | Self::RESOURCE.0
            | Self::SCENE.0
            | Self::EDITOR.0,
    );

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl std::ops::BitOr for EventCategoryMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for EventCategoryMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    WindowResized {
        width: u32,
        height: u32,
    },
    KeyPressed {
        key_code: u32,
        repeat: bool,
    },
    KeyReleased {
        key_code: u32,
    },
    MouseMoved {
        position: [f32; 2],
    },
    CollisionStarted {
        entity_a: u64,
        entity_b: u64,
    },
    CollisionEnded {
        entity_a: u64,
        entity_b: u64,
    },
    ResourceLoaded {
        kind: ResourceEventKind,
        resource_id: String,
    },
    ResourceReloaded {
        kind: ResourceEventKind,
        resource_id: String,
    },
    ResourceFailed {
        kind: ResourceEventKind,
        resource_id: String,
        reason: String,
    },
    SceneLoaded {
        scene_name: String,
    },
    EntitySelected {
        entity: Option<u64>,
    },
    EntityTransformed {
        entity: u64,
    },
    PlayModeChanged {
        playing: bool,
    },
}

impl EngineEvent {
    pub fn category_mask(&self) -> EventCategoryMask {
        match self {
            Self::WindowResized { .. } => {
                EventCategoryMask::APPLICATION | EventCategoryMask::WINDOW
            }
            Self::KeyPressed { .. } | Self::KeyReleased { .. } | Self::MouseMoved { .. } => {
                EventCategoryMask::APPLICATION | EventCategoryMask::INPUT
            }
            Self::CollisionStarted { .. } | Self::CollisionEnded { .. } => {
                EventCategoryMask::PHYSICS
            }
            Self::ResourceLoaded { .. }
            | Self::ResourceReloaded { .. }
            | Self::ResourceFailed { .. } => EventCategoryMask::RESOURCE,
            Self::SceneLoaded { .. } => EventCategoryMask::SCENE,
            Self::EntitySelected { .. }
            | Self::EntityTransformed { .. }
            | Self::PlayModeChanged { .. } => EventCategoryMask::EDITOR | EventCategoryMask::SCENE,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::WindowResized { .. } => "WindowResized",
            Self::KeyPressed { .. } => "KeyPressed",
            Self::KeyReleased { .. } => "KeyReleased",
            Self::MouseMoved { .. } => "MouseMoved",
            Self::CollisionStarted { .. } => "CollisionStarted",
            Self::CollisionEnded { .. } => "CollisionEnded",
            Self::ResourceLoaded { .. } => "ResourceLoaded",
            Self::ResourceReloaded { .. } => "ResourceReloaded",
            Self::ResourceFailed { .. } => "ResourceFailed",
            Self::SceneLoaded { .. } => "SceneLoaded",
            Self::EntitySelected { .. } => "EntitySelected",
            Self::EntityTransformed { .. } => "EntityTransformed",
            Self::PlayModeChanged { .. } => "PlayModeChanged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDeliveryMode {
    Immediate,
    Queued,
}

pub struct EventSubscription {
    pub categories: EventCategoryMask,
}

impl Default for EventSubscription {
    fn default() -> Self {
        Self {
            categories: EventCategoryMask::ALL,
        }
    }
}

struct ListenerEntry {
    categories: EventCategoryMask,
    callback: Box<dyn FnMut(&EngineEvent) + Send>,
}

pub struct EventBus {
    next_listener_id: u64,
    listeners: HashMap<EventListenerHandle, ListenerEntry>,
    queued_events: VecDeque<EngineEvent>,
    event_trace: Vec<EngineEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            next_listener_id: 1,
            listeners: HashMap::new(),
            queued_events: VecDeque::new(),
            event_trace: Vec::new(),
        }
    }
}

impl EventBus {
    pub fn subscribe(
        &mut self,
        subscription: EventSubscription,
        callback: impl FnMut(&EngineEvent) + Send + 'static,
    ) -> EventListenerHandle {
        let handle = EventListenerHandle(self.next_listener_id);
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        self.listeners.insert(
            handle,
            ListenerEntry {
                categories: subscription.categories,
                callback: Box::new(callback),
            },
        );
        handle
    }

    pub fn unsubscribe(&mut self, handle: EventListenerHandle) -> bool {
        self.listeners.remove(&handle).is_some()
    }

    pub fn publish(&mut self, event: EngineEvent, mode: EventDeliveryMode) {
        self.event_trace.push(event.clone());
        match mode {
            EventDeliveryMode::Immediate => self.dispatch(&event),
            EventDeliveryMode::Queued => self.queued_events.push_back(event),
        }
    }

    pub fn process_queued(&mut self) {
        while let Some(event) = self.queued_events.pop_front() {
            self.dispatch(&event);
        }
    }

    pub fn queued_len(&self) -> usize {
        self.queued_events.len()
    }

    pub fn trace(&self) -> &[EngineEvent] {
        &self.event_trace
    }

    pub fn clear_trace(&mut self) {
        self.event_trace.clear();
    }

    fn dispatch(&mut self, event: &EngineEvent) {
        let categories = event.category_mask();
        for listener in self.listeners.values_mut() {
            if listener.categories.intersects(categories) {
                (listener.callback)(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn event_bus_dispatches_immediate_events() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mut bus = EventBus::default();
        let sink = received.clone();
        bus.subscribe(EventSubscription::default(), move |event| {
            sink.lock().expect("sink lock").push(event.kind_name());
        });

        bus.publish(
            EngineEvent::WindowResized {
                width: 1280,
                height: 720,
            },
            EventDeliveryMode::Immediate,
        );

        assert_eq!(*received.lock().expect("sink lock"), vec!["WindowResized"]);
    }

    #[test]
    fn event_bus_queues_and_flushes_events() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mut bus = EventBus::default();
        let sink = received.clone();
        bus.subscribe(EventSubscription::default(), move |event| {
            sink.lock().expect("sink lock").push(event.kind_name());
        });

        bus.publish(
            EngineEvent::SceneLoaded {
                scene_name: "test".to_string(),
            },
            EventDeliveryMode::Queued,
        );
        assert_eq!(bus.queued_len(), 1);
        assert!(received.lock().expect("sink lock").is_empty());

        bus.process_queued();
        assert_eq!(*received.lock().expect("sink lock"), vec!["SceneLoaded"]);
    }

    #[test]
    fn event_bus_filters_by_category() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mut bus = EventBus::default();
        let sink = received.clone();
        bus.subscribe(
            EventSubscription {
                categories: EventCategoryMask::RESOURCE,
            },
            move |event| {
                sink.lock().expect("sink lock").push(event.kind_name());
            },
        );

        bus.publish(
            EngineEvent::KeyPressed {
                key_code: 87,
                repeat: false,
            },
            EventDeliveryMode::Immediate,
        );
        bus.publish(
            EngineEvent::ResourceLoaded {
                kind: ResourceEventKind::Texture,
                resource_id: "crate".to_string(),
            },
            EventDeliveryMode::Immediate,
        );

        assert_eq!(*received.lock().expect("sink lock"), vec!["ResourceLoaded"]);
    }

    #[test]
    fn event_bus_unsubscribe_stops_delivery() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mut bus = EventBus::default();
        let sink = received.clone();
        let handle = bus.subscribe(EventSubscription::default(), move |event| {
            sink.lock().expect("sink lock").push(event.kind_name());
        });
        assert!(bus.unsubscribe(handle));
        bus.publish(
            EngineEvent::PlayModeChanged { playing: true },
            EventDeliveryMode::Immediate,
        );
        assert!(received.lock().expect("sink lock").is_empty());
    }
}
