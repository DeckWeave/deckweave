use super::{AudioCommand, AudioState};
use ::windows::Win32::{
    Media::Audio::{
        Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, MMDeviceEnumerator, eMultimedia,
        eRender,
    },
    System::Com::{
        CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
        CoUninitialize,
    },
};
use std::{marker::PhantomData, rc::Rc};

/// Must be constructed, used and dropped on one thread. No COM pointers cross the boundary.
pub struct AudioSession {
    _thread_affinity: PhantomData<Rc<()>>,
}

impl AudioSession {
    pub fn new() -> Result<Self, String> {
        // SAFETY: helper owns its dedicated main thread. Every successful initialize (including
        // S_FALSE) is paired with one uninitialize by Drop; errors never construct a session.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
            .map_err(|e| format!("Could not initialize audio COM: {e}"))?;
        Ok(Self {
            _thread_affinity: PhantomData,
        })
    }

    pub fn execute(&self, command: AudioCommand) -> Result<AudioState, String> {
        let command = command.validate()?;
        // SAFETY: COM lives on this thread for self's lifetime. The typed interfaces own their
        // references. Resolve the default endpoint each request: unplug, default-output changes
        // and audio-service recovery never reuse an invalid endpoint. No retained raw pointers.
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| format!("Audio service unavailable: {e}"))?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(|e| format!("No default audio output: {e}"))?;
            let endpoint: IAudioEndpointVolume = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Audio output unavailable: {e}"))?;
            let result = (|| -> ::windows::core::Result<AudioState> {
                match command {
                    AudioCommand::Status => {}
                    AudioCommand::SetVolume { value } => {
                        endpoint.SetMasterVolumeLevelScalar(value / 100.0, std::ptr::null())?
                    }
                    AudioCommand::AdjustVolume { delta } => {
                        let current = endpoint.GetMasterVolumeLevelScalar()?;
                        endpoint.SetMasterVolumeLevelScalar(
                            (current + delta / 100.0).clamp(0.0, 1.0),
                            std::ptr::null(),
                        )?;
                    }
                    AudioCommand::ToggleMute => {
                        endpoint.SetMute(!endpoint.GetMute()?.as_bool(), std::ptr::null())?
                    }
                }
                let id = device.GetId()?;
                let endpoint_id = id.to_string();
                // GetId allocates with CoTaskMemAlloc. Copy first, free exactly once, including
                // the malformed-string path; the resulting Rust string owns its memory.
                CoTaskMemFree(Some(id.as_ptr().cast()));
                Ok(AudioState {
                    volume: endpoint.GetMasterVolumeLevelScalar()? * 100.0,
                    muted: endpoint.GetMute()?.as_bool(),
                    endpoint: endpoint_id?,
                })
            })();
            result.map_err(|e| format!("Could not control audio output: {e}"))
        }
    }
}

impl Drop for AudioSession {
    fn drop(&mut self) {
        // SAFETY: thread-affine session cannot be sent to another thread; new initialized COM.
        unsafe { CoUninitialize() }
    }
}
