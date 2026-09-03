use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use image::RgbaImage;
use tbx_core::batch::BatchStep;
use tbx_core::maps::{MapOutputs, MapSetParams};
use tbx_entitlements::EntitlementGate;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Source,
    Tileable,
    Maps,
    ChannelPack,
    Atlas,
    Optimize,
    Preview,
    Batch,
    Presets,
}
#[derive(Clone, Debug)]
pub struct ImageSlot {
    pub image: Arc<RgbaImage>,
    pub name: String,
    pub origin: String,
}
#[derive(Clone, Debug, Default)]
pub struct MapsState {
    pub params: MapSetParams,
    pub outputs: Option<MapOutputs>,
}
#[derive(Debug, Default)]
pub struct ProjectState {
    pub source: Option<ImageSlot>,
    pub tileable: Option<ImageSlot>,
    pub maps: MapsState,
    pub packed: Option<ImageSlot>,
    pub atlas: Option<ImageSlot>,
    pub finalized: Option<ImageSlot>,
    pub batch_chain: Vec<BatchStep>,
}
#[derive(Clone, Debug)]
pub enum AppEvent {
    StageUpdated(Stage),
    PlanChanged,
    LanguageChanged,
}
#[derive(Clone, Default)]
pub struct EventBus {
    subs: Arc<Mutex<Vec<Sender<AppEvent>>>>,
}
impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn subscribe(&self) -> Receiver<AppEvent> {
        let (tx, rx) = channel();
        self.subs.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        rx
    }
    pub fn publish(&self, event: AppEvent) {
        let mut subs = self.subs.lock().unwrap_or_else(|e| e.into_inner());
        subs.retain(|tx| tx.send(event.clone()).is_ok());
    }
}
pub struct AppState {
    pub project: Arc<RwLock<ProjectState>>,
    pub gate: Arc<EntitlementGate>,
    pub bus: EventBus,
}
impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            project: Arc::new(RwLock::new(ProjectState::default())),
            gate: EntitlementGate::new(),
            bus: EventBus::new(),
        })
    }
    pub fn update<F: FnOnce(&mut ProjectState)>(&self, stage: Stage, f: F) {
        {
            let mut state = self.project.write().unwrap_or_else(|e| e.into_inner());
            f(&mut state);
        }
        self.bus.publish(AppEvent::StageUpdated(stage));
    }
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            project: Arc::new(RwLock::new(ProjectState::default())),
            gate: EntitlementGate::new(),
            bus: EventBus::new(),
        }
    }
}
