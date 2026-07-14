#![cfg_attr(not(target_os = "android"), forbid(unsafe_code))]

#[cfg(target_os = "android")]
mod android {
    use std::ffi::{CString, c_char, c_int};
    use std::sync::Once;

    use physbox_engine::EngineConfig;
    use physbox_launch::{ANDROID_STARTUP_ARGV_INTENT_EXTRA, LaunchOptions, parse_android_json};
    use physbox_shell::WinitFrameDriver;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::platform::android::activity::AndroidApp;
    use winit::window::{WindowAttributes, WindowId};

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_print(
            priority: c_int,
            tag: *const c_char,
            format: *const c_char,
            ...
        ) -> c_int;
    }

    struct AndroidLogger;

    impl log::Log for AndroidLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let priority = match record.level() {
                log::Level::Error => 6,
                log::Level::Warn => 5,
                log::Level::Info => 4,
                log::Level::Debug => 3,
                log::Level::Trace => 2,
            };
            let tag = CString::new("physbox5").unwrap();
            let message = CString::new(record.args().to_string().replace('\0', "\\0")).unwrap();
            unsafe {
                __android_log_print(priority, tag.as_ptr(), c"%s".as_ptr(), message.as_ptr());
            }
        }

        fn flush(&self) {}
    }

    static LOGGER: AndroidLogger = AndroidLogger;
    static INIT_LOGGER: Once = Once::new();

    fn init_logger() {
        INIT_LOGGER.call_once(|| {
            if log::set_logger(&LOGGER).is_ok() {
                log::set_max_level(log::LevelFilter::Info);
            }
        });
    }

    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        init_logger();
        let options = startup_options(&app).unwrap_or_else(|error| {
            log::error!("PHYSBOX5_ANDROID_STARTUP_ERROR {error}");
            LaunchOptions::default()
        });
        let event_loop = EventLoop::builder()
            .with_android_app(app)
            .build()
            .expect("failed to create Android event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut host = AndroidHost {
            driver: WinitFrameDriver::new(EngineConfig::from(&options)),
        };
        event_loop
            .run_app(&mut host)
            .expect("PhysBox 5 Android event loop failed");
    }

    struct AndroidHost {
        driver: WinitFrameDriver,
    }

    impl ApplicationHandler for AndroidHost {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            match self.driver.resume(
                event_loop,
                WindowAttributes::default().with_title("PhysBox 5"),
            ) {
                Ok(Some(adapter)) => {
                    log::info!("PHYSBOX5_MARK_ADAPTER {}", adapter.marker_json())
                }
                Ok(None) => {}
                Err(error) => log::error!("PHYSBOX5_ANDROID_RENDER_ERROR {error}"),
            }
        }

        fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
            self.driver.suspend();
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            match self.driver.handle_window_event(window_id, event) {
                Ok(outcome) => {
                    if let Some(report) = outcome.report
                        && (report.frame_index == 1 || report.scenario.complete)
                    {
                        log::info!("PHYSBOX5_MARK_FRAME_RENDERED {}", report.marker_json());
                    }
                    if outcome.exit_requested {
                        event_loop.exit();
                    }
                }
                Err(error) => log::error!("PHYSBOX5_ANDROID_RENDER_ERROR {error}"),
            }
        }
    }

    fn startup_options(app: &AndroidApp) -> Result<LaunchOptions, String> {
        let json = android_startup_argv_json(app)?;
        parse_android_json(json.as_deref().unwrap_or("[]"))
            .map(|request| request.options)
            .map_err(|error| error.to_string())
    }

    fn android_startup_argv_json(app: &AndroidApp) -> Result<Option<String>, String> {
        let vm = app.vm_as_ptr() as *mut jni::sys::JavaVM;
        let activity = app.activity_as_ptr() as jni::sys::jobject;
        if vm.is_null() || activity.is_null() {
            return Err("null JVM or NativeActivity while reading startup intent".to_owned());
        }
        let vm = unsafe { jni::JavaVM::from_raw(vm) };
        vm.attach_current_thread(|env| {
            let activity = unsafe { jni::objects::JObject::from_raw(env, activity) };
            let intent = env
                .call_method(
                    &activity,
                    jni::jni_str!("getIntent"),
                    jni::jni_sig!("()Landroid/content/Intent;"),
                    &[],
                )?
                .into_object()?;
            if intent.is_null() {
                return Ok(None);
            }
            let key =
                jni::objects::JObject::from(env.new_string(ANDROID_STARTUP_ARGV_INTENT_EXTRA)?);
            let value = env
                .call_method(
                    &intent,
                    jni::jni_str!("getStringExtra"),
                    jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[jni::objects::JValue::Object(&key)],
                )?
                .into_object()?;
            if value.is_null() {
                return Ok(None);
            }
            let value = jni::objects::JString::cast_local(env, value)?;
            value.try_to_string(env).map(Some)
        })
        .map_err(|error| {
            format!("read Android intent extra `{ANDROID_STARTUP_ARGV_INTENT_EXTRA}`: {error}")
        })
    }
}

#[cfg(not(target_os = "android"))]
pub fn host_placeholder() {}
