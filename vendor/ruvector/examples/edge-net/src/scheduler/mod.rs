//! Idle detection and CPU throttling for non-intrusive compute contribution

use wasm_bindgen::prelude::*;

/// Idle detection and throttling
#[wasm_bindgen]
pub struct WasmIdleDetector {
    /// Maximum CPU usage (0.0 - 1.0)
    max_cpu: f32,
    /// Minimum idle time before contributing (ms)
    min_idle_time: u32,
    /// Whether detector is active
    active: bool,
    /// Whether paused by user
    paused: bool,
    /// Last user interaction timestamp
    last_interaction: u64,
    /// Is on battery power
    on_battery: bool,
    /// Respect battery saver
    respect_battery: bool,
    /// Current frame rate
    current_fps: f32,
    /// Target FPS minimum
    target_fps: f32,
}

#[wasm_bindgen]
impl WasmIdleDetector {
    /// Create a new idle detector
    #[wasm_bindgen(constructor)]
    pub fn new(max_cpu: f32, min_idle_time: u32) -> Result<WasmIdleDetector, JsValue> {
        Ok(WasmIdleDetector {
            max_cpu: max_cpu.clamp(0.0, 1.0),
            min_idle_time,
            active: false,
            paused: false,
            last_interaction: js_sys::Date::now() as u64,
            on_battery: false,
            respect_battery: true,
            current_fps: 60.0,
            target_fps: 30.0, // Minimum acceptable FPS
        })
    }

    /// Start monitoring
    #[wasm_bindgen]
    pub fn start(&mut self) -> Result<(), JsValue> {
        self.active = true;
        self.update_battery_status()?;
        Ok(())
    }

    /// Stop monitoring
    #[wasm_bindgen]
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Pause contribution (user-initiated)
    #[wasm_bindgen]
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume contribution
    #[wasm_bindgen]
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Check if user is idle
    #[wasm_bindgen(js_name = isIdle)]
    pub fn is_idle(&self) -> bool {
        let now = js_sys::Date::now() as u64;
        let idle_duration = now - self.last_interaction;

        idle_duration > self.min_idle_time as u64
    }

    /// Check if we should be working
    #[wasm_bindgen(js_name = shouldWork)]
    pub fn should_work(&self) -> bool {
        if !self.active || self.paused {
            return false;
        }

        // Don't work if on battery and battery saver is respected
        if self.on_battery && self.respect_battery {
            return false;
        }

        // Don't work if FPS is too low (page is struggling)
        if self.current_fps < self.target_fps {
            return false;
        }

        true
    }

    /// Get current throttle level (0.0 - max_cpu)
    #[wasm_bindgen(js_name = getThrottle)]
    pub fn get_throttle(&self) -> f32 {
        if !self.should_work() {
            return 0.0;
        }

        // Reduce throttle if FPS is getting low
        let fps_factor = if self.current_fps < 60.0 {
            (self.current_fps - self.target_fps) / (60.0 - self.target_fps)
        } else {
            1.0
        };

        // Reduce throttle if recently active
        let idle_factor = if self.is_idle() {
            1.0
        } else {
            0.3 // Only use 30% when user is active
        };

        self.max_cpu * fps_factor.clamp(0.0, 1.0) * idle_factor
    }

    /// Record user interaction
    #[wasm_bindgen(js_name = recordInteraction)]
    pub fn record_interaction(&mut self) {
        self.last_interaction = js_sys::Date::now() as u64;
    }

    /// Update FPS measurement
    #[wasm_bindgen(js_name = updateFps)]
    pub fn update_fps(&mut self, fps: f32) {
        // Smooth FPS with exponential moving average
        self.current_fps = self.current_fps * 0.9 + fps * 0.1;
    }

    /// Update battery status
    fn update_battery_status(&mut self) -> Result<(), JsValue> {
        // Would use navigator.getBattery() in JS
        // For now, default to not on battery
        self.on_battery = false;
        Ok(())
    }

    /// Set battery status (called from JS)
    #[wasm_bindgen(js_name = setBatteryStatus)]
    pub fn set_battery_status(&mut self, on_battery: bool) {
        self.on_battery = on_battery;
    }

    /// Get status summary
    #[wasm_bindgen(js_name = getStatus)]
    pub fn get_status(&self) -> JsValue {
        let obj = js_sys::Object::new();

        js_sys::Reflect::set(&obj, &"active".into(), &self.active.into()).unwrap();
        js_sys::Reflect::set(&obj, &"paused".into(), &self.paused.into()).unwrap();
        js_sys::Reflect::set(&obj, &"idle".into(), &self.is_idle().into()).unwrap();
        js_sys::Reflect::set(&obj, &"shouldWork".into(), &self.should_work().into()).unwrap();
        js_sys::Reflect::set(&obj, &"throttle".into(), &self.get_throttle().into()).unwrap();
        js_sys::Reflect::set(&obj, &"fps".into(), &self.current_fps.into()).unwrap();
        js_sys::Reflect::set(&obj, &"onBattery".into(), &self.on_battery.into()).unwrap();

        obj.into()
    }
}

/// Work scheduler for distributing compute across frames
#[wasm_bindgen]
pub struct WasmWorkScheduler {
    /// Tasks queued for execution
    pending_tasks: usize,
    /// Maximum tasks per frame
    max_per_frame: usize,
    /// Time budget per frame (ms)
    time_budget_ms: f64,
    /// Average task duration (ms)
    avg_task_duration_ms: f64,
}

#[wasm_bindgen]
impl WasmWorkScheduler {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmWorkScheduler {
        WasmWorkScheduler {
            pending_tasks: 0,
            max_per_frame: 5,
            time_budget_ms: 4.0, // ~1/4 of 16ms frame
            avg_task_duration_ms: 1.0,
        }
    }

    /// Calculate how many tasks to run this frame
    #[wasm_bindgen(js_name = tasksThisFrame)]
    pub fn tasks_this_frame(&self, throttle: f32) -> usize {
        if throttle <= 0.0 {
            return 0;
        }

        // Calculate based on time budget
        let budget = self.time_budget_ms * throttle as f64;
        let count = (budget / self.avg_task_duration_ms) as usize;

        count.min(self.max_per_frame).min(self.pending_tasks)
    }

    /// Record task completion for averaging
    #[wasm_bindgen(js_name = recordTaskDuration)]
    pub fn record_task_duration(&mut self, duration_ms: f64) {
        // Exponential moving average
        self.avg_task_duration_ms = self.avg_task_duration_ms * 0.9 + duration_ms * 0.1;
    }

    /// Set pending task count
    #[wasm_bindgen(js_name = setPendingTasks)]
    pub fn set_pending_tasks(&mut self, count: usize) {
        self.pending_tasks = count;
    }
}
