//! MiniMax text-to-audio provider for the HOMECORE TTS contract.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::audio::{AudioCodec, AudioFormat};
use crate::speech::{SpeechError, SynthesizedSpeech, TextToSpeech};

pub const MINIMAX_TTS_MODELS: &[&str] = &[
    "speech-2.8-hd",
    "speech-2.8-turbo",
    "speech-2.6-hd",
    "speech-2.6-turbo",
    "speech-02-hd",
    "speech-02-turbo",
    "speech-01-hd",
    "speech-01-turbo",
];

pub const MINIMAX_TTS_DEFAULT_MODEL: &str = "speech-2.8-hd";
pub const MINIMAX_TTS_INLINE_OUTPUT_FORMAT: &str = "hex";

pub const MINIMAX_TTS_ENDPOINTS: &[(&str, &str)] = &[
    ("global_en", "https://api.minimax.io/v1/t2a_v2"),
    ("cn_zh", "https://api.minimaxi.com/v1/t2a_v2"),
];

pub const MINIMAX_TTS_DOCS_URLS: &[&str] = &[
    "https://platform.minimax.io/docs/api-reference/speech-t2a-http",
    "https://platform.minimax.io/docs/api-reference/speech-t2a-async-create",
    "https://platform.minimax.io/docs/api-reference/speech-t2a-websocket",
    "https://platform.minimaxi.com/docs/api-reference/speech-t2a-http",
    "https://platform.minimaxi.com/docs/api-reference/speech-t2a-async-create",
    "https://platform.minimaxi.com/docs/api-reference/speech-t2a-websocket",
];

pub const MINIMAX_TTS_REQUEST_FIELDS: &[&str] = &[
    "model",
    "text",
    "stream",
    "language_boost",
    "output_format",
    "voice_setting",
    "pronunciation_dict",
    "audio_setting",
    "voice_modify",
    "subtitle_enable",
];

pub const MINIMAX_TTS_REQUIRED_FIELDS: &[&str] = &["model", "text"];

pub const MINIMAX_TTS_AUDIO_FORMATS: &[MiniMaxAudioOutputFormat] = &[
    MiniMaxAudioOutputFormat::Mp3,
    MiniMaxAudioOutputFormat::Wav,
    MiniMaxAudioOutputFormat::Flac,
    MiniMaxAudioOutputFormat::Pcm,
];

pub const MINIMAX_TTS_RESPONSE_FIELDS: &[&str] =
    &["data.audio", "data.status", "base_resp.status_code"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MiniMaxRegion {
    GlobalEn,
    CnZh,
}

impl MiniMaxRegion {
    pub fn endpoint(self) -> &'static str {
        match self {
            Self::GlobalEn => "https://api.minimax.io/v1/t2a_v2",
            Self::CnZh => "https://api.minimaxi.com/v1/t2a_v2",
        }
    }

    pub fn docs_root(self) -> &'static str {
        match self {
            Self::GlobalEn => "https://platform.minimax.io/docs",
            Self::CnZh => "https://platform.minimaxi.com/docs",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MiniMaxAudioOutputFormat {
    Mp3,
    Wav,
    Flac,
    Pcm,
}

impl MiniMaxAudioOutputFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Pcm => "pcm",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MiniMaxPronunciationDict {
    pub tone: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiniMaxTtsConfig {
    pub api_key: String,
    pub region: MiniMaxRegion,
    pub model: String,
    pub voice_id: Option<String>,
    pub language_boost: Option<String>,
    pub output_format: MiniMaxAudioOutputFormat,
    pub pronunciation_dict: Option<MiniMaxPronunciationDict>,
    pub subtitle_enable: bool,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub timeout: Duration,
}

impl MiniMaxTtsConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            region: MiniMaxRegion::GlobalEn,
            model: MINIMAX_TTS_DEFAULT_MODEL.into(),
            voice_id: None,
            language_boost: None,
            output_format: MiniMaxAudioOutputFormat::Pcm,
            pronunciation_dict: None,
            subtitle_enable: false,
            sample_rate: 16_000,
            channels: 1,
            bitrate: 128_000,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_region(mut self, region: MiniMaxRegion) -> Self {
        self.region = region;
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_voice_id(mut self, voice_id: impl Into<String>) -> Self {
        self.voice_id = Some(voice_id.into());
        self
    }

    pub fn with_language_boost(mut self, language_boost: impl Into<String>) -> Self {
        self.language_boost = Some(language_boost.into());
        self
    }

    pub fn with_output_format(mut self, output_format: MiniMaxAudioOutputFormat) -> Self {
        self.output_format = output_format;
        self
    }

    pub fn with_pronunciation_dict(mut self, pronunciation_dict: MiniMaxPronunciationDict) -> Self {
        self.pronunciation_dict = Some(pronunciation_dict);
        self
    }

    pub fn with_subtitles_enabled(mut self, subtitle_enable: bool) -> Self {
        self.subtitle_enable = subtitle_enable;
        self
    }

    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.api_key.trim().is_empty() {
            return Err(SpeechError::NotConfigured("MiniMax TTS"));
        }
        if !MINIMAX_TTS_MODELS.contains(&self.model.as_str()) {
            return Err(SpeechError::Provider(format!(
                "unsupported MiniMax speech model: {}",
                self.model
            )));
        }
        // HOMECORE consumes raw PCM16; compressed MiniMax formats remain discovery metadata.
        if self.output_format != MiniMaxAudioOutputFormat::Pcm {
            return Err(SpeechError::Provider(
                "HOMECORE voice output requires pcm audio".into(),
            ));
        }
        if ![8_000, 16_000, 22_050, 24_000, 32_000, 44_100].contains(&self.sample_rate) {
            return Err(SpeechError::Provider(format!(
                "unsupported MiniMax sample rate: {}",
                self.sample_rate
            )));
        }
        AudioFormat {
            codec: AudioCodec::PcmS16Le,
            sample_rate: self.sample_rate,
            channels: self.channels,
        }
        .validate()
        .map_err(|err| SpeechError::Provider(err.to_string()))?;
        Ok(())
    }
}

pub struct MiniMaxTts {
    config: MiniMaxTtsConfig,
}

impl MiniMaxTts {
    pub fn new(config: MiniMaxTtsConfig) -> Result<Self, SpeechError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &MiniMaxTtsConfig {
        &self.config
    }

    fn synthesize_blocking(
        config: &MiniMaxTtsConfig,
        text: &str,
        language: &str,
    ) -> Result<SynthesizedSpeech, SpeechError> {
        let request = build_tts_request(config, text, language);
        let body = serde_json::to_string(&request)
            .map_err(|err| SpeechError::Provider(err.to_string()))?;
        let response = ureq::AgentBuilder::new()
            .timeout(config.timeout)
            .build()
            .post(config.region.endpoint())
            .set("Authorization", &format!("Bearer {}", config.api_key))
            .set("Content-Type", "application/json")
            .send_string(&body);

        let response = match response {
            Ok(response) => response,
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                return Err(SpeechError::Provider(format!(
                    "MiniMax TTS returned HTTP {code}: {body}"
                )));
            }
            Err(ureq::Error::Transport(err)) => {
                return Err(SpeechError::Provider(err.to_string()));
            }
        };

        let body = response
            .into_string()
            .map_err(|err| SpeechError::InvalidOutput(err.to_string()))?;
        parse_tts_response(&body, config.sample_rate, config.channels)
    }
}

#[async_trait]
impl TextToSpeech for MiniMaxTts {
    async fn synthesize(
        &self,
        text: &str,
        language: &str,
    ) -> Result<SynthesizedSpeech, SpeechError> {
        if text.trim().is_empty() {
            return Err(SpeechError::Provider("text is required".into()));
        }
        let config = self.config.clone();
        let text = text.to_owned();
        let language = language.to_owned();
        tokio::task::spawn_blocking(move || Self::synthesize_blocking(&config, &text, &language))
            .await
            .map_err(|error| SpeechError::Provider(format!("MiniMax TTS task failed: {error}")))?
    }
}

#[derive(Debug, Serialize)]
struct MiniMaxTtsRequest<'a> {
    model: &'a str,
    text: &'a str,
    stream: bool,
    language_boost: &'a str,
    output_format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice_setting: Option<MiniMaxVoiceSetting<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pronunciation_dict: Option<&'a MiniMaxPronunciationDict>,
    audio_setting: MiniMaxAudioSetting,
    subtitle_enable: bool,
}

#[derive(Debug, Serialize)]
struct MiniMaxVoiceSetting<'a> {
    voice_id: &'a str,
}

#[derive(Debug, Serialize)]
struct MiniMaxAudioSetting {
    sample_rate: u32,
    bitrate: u32,
    channel: u8,
    format: &'static str,
}

fn build_tts_request<'a>(
    config: &'a MiniMaxTtsConfig,
    text: &'a str,
    language: &'a str,
) -> MiniMaxTtsRequest<'a> {
    MiniMaxTtsRequest {
        model: &config.model,
        text,
        stream: false,
        language_boost: config
            .language_boost
            .as_deref()
            .unwrap_or_else(|| normalize_language_boost(language)),
        output_format: MINIMAX_TTS_INLINE_OUTPUT_FORMAT,
        voice_setting: config
            .voice_id
            .as_deref()
            .map(|voice_id| MiniMaxVoiceSetting { voice_id }),
        pronunciation_dict: config.pronunciation_dict.as_ref(),
        audio_setting: MiniMaxAudioSetting {
            sample_rate: config.sample_rate,
            bitrate: config.bitrate,
            channel: config.channels,
            format: config.output_format.as_str(),
        },
        subtitle_enable: config.subtitle_enable,
    }
}

fn normalize_language_boost(language: &str) -> &'static str {
    let normalized = language.trim().to_ascii_lowercase().replace('_', "-");
    if normalized == "yue" || normalized.starts_with("yue-") || normalized.starts_with("zh-yue") {
        return "Chinese,Yue";
    }
    match normalized.split('-').next().unwrap_or_default() {
        "zh" => "Chinese",
        "en" => "English",
        "ar" => "Arabic",
        "ru" => "Russian",
        "es" => "Spanish",
        "fr" => "French",
        "pt" => "Portuguese",
        "de" => "German",
        "tr" => "Turkish",
        "nl" => "Dutch",
        "uk" => "Ukrainian",
        "vi" => "Vietnamese",
        "id" => "Indonesian",
        "ja" => "Japanese",
        "it" => "Italian",
        "ko" => "Korean",
        "th" => "Thai",
        "pl" => "Polish",
        "ro" => "Romanian",
        "el" => "Greek",
        "cs" => "Czech",
        "fi" => "Finnish",
        "hi" => "Hindi",
        "bg" => "Bulgarian",
        "da" => "Danish",
        "he" => "Hebrew",
        "ms" => "Malay",
        "fa" => "Persian",
        "sk" => "Slovak",
        "sv" => "Swedish",
        "hr" => "Croatian",
        "fil" | "tl" => "Filipino",
        "hu" => "Hungarian",
        "no" => "Norwegian",
        "sl" => "Slovenian",
        "ca" => "Catalan",
        "nn" => "Nynorsk",
        "ta" => "Tamil",
        "af" => "Afrikaans",
        _ => "auto",
    }
}

#[derive(Debug, Deserialize)]
struct MiniMaxTtsResponse {
    data: Option<MiniMaxTtsData>,
    base_resp: Option<MiniMaxBaseResponse>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxTtsData {
    audio: Option<String>,
    status: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct MiniMaxBaseResponse {
    status_code: Option<i64>,
    status_msg: Option<String>,
}

fn parse_tts_response(
    body: &str,
    sample_rate: u32,
    channels: u8,
) -> Result<SynthesizedSpeech, SpeechError> {
    let response: MiniMaxTtsResponse =
        serde_json::from_str(body).map_err(|err| SpeechError::InvalidOutput(err.to_string()))?;
    let base_resp = response
        .base_resp
        .ok_or_else(|| SpeechError::InvalidOutput("missing base_resp".into()))?;
    let status_code = base_resp
        .status_code
        .ok_or_else(|| SpeechError::InvalidOutput("missing base_resp.status_code".into()))?;
    if status_code != 0 {
        return Err(SpeechError::Provider(base_resp.status_msg.unwrap_or_else(
            || format!("MiniMax TTS request failed with code {status_code}"),
        )));
    }

    let data = response
        .data
        .ok_or_else(|| SpeechError::InvalidOutput("missing data".into()))?;
    if data.status != Some(2) {
        return Err(SpeechError::InvalidOutput(
            "data.status must indicate completed synthesis".into(),
        ));
    }
    let audio_hex = data
        .audio
        .ok_or_else(|| SpeechError::InvalidOutput("missing data.audio".into()))?;
    let audio = decode_hex_audio(&audio_hex)?;
    crate::speech::validate_provider_audio(&audio)?;
    Ok(SynthesizedSpeech {
        audio,
        format: AudioFormat {
            codec: AudioCodec::PcmS16Le,
            sample_rate,
            channels,
        },
    })
}

fn decode_hex_audio(input: &str) -> Result<Vec<u8>, SpeechError> {
    let hex = input.trim();
    if hex.len() % 2 != 0 {
        return Err(SpeechError::InvalidOutput(
            "data.audio hex length must be even".into(),
        ));
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_nibble(byte: u8) -> Result<u8, SpeechError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(SpeechError::InvalidOutput(
            "data.audio must be hex encoded".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_current_minimax_tts_models_regions_and_fields() {
        assert_eq!(MINIMAX_TTS_DEFAULT_MODEL, "speech-2.8-hd");
        assert_eq!(
            MINIMAX_TTS_MODELS,
            [
                "speech-2.8-hd",
                "speech-2.8-turbo",
                "speech-2.6-hd",
                "speech-2.6-turbo",
                "speech-02-hd",
                "speech-02-turbo",
                "speech-01-hd",
                "speech-01-turbo",
            ]
        );
        assert_eq!(
            MINIMAX_TTS_ENDPOINTS,
            [
                ("global_en", "https://api.minimax.io/v1/t2a_v2"),
                ("cn_zh", "https://api.minimaxi.com/v1/t2a_v2"),
            ]
        );
        assert_eq!(MINIMAX_TTS_REQUIRED_FIELDS, ["model", "text"]);
        assert_eq!(
            MINIMAX_TTS_REQUEST_FIELDS,
            [
                "model",
                "text",
                "stream",
                "language_boost",
                "output_format",
                "voice_setting",
                "pronunciation_dict",
                "audio_setting",
                "voice_modify",
                "subtitle_enable",
            ]
        );
        assert_eq!(
            MINIMAX_TTS_AUDIO_FORMATS,
            [
                MiniMaxAudioOutputFormat::Mp3,
                MiniMaxAudioOutputFormat::Wav,
                MiniMaxAudioOutputFormat::Flac,
                MiniMaxAudioOutputFormat::Pcm,
            ]
        );
        assert_eq!(
            MINIMAX_TTS_RESPONSE_FIELDS,
            ["data.audio", "data.status", "base_resp.status_code"]
        );
    }

    #[test]
    fn builds_http_request_with_supported_fields_and_hex_response_encoding() {
        let config = MiniMaxTtsConfig::new("test-key")
            .with_region(MiniMaxRegion::CnZh)
            .with_model("speech-2.8-turbo")
            .with_voice_id("voice-123")
            .with_language_boost("Chinese,Yue")
            .with_output_format(MiniMaxAudioOutputFormat::Pcm)
            .with_pronunciation_dict(MiniMaxPronunciationDict {
                tone: vec!["read/(riːd)".into()],
            })
            .with_subtitles_enabled(true);

        let request = build_tts_request(&config, "hello", "yue-HK");
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(
            config.region.endpoint(),
            "https://api.minimaxi.com/v1/t2a_v2"
        );
        assert_eq!(json["model"], "speech-2.8-turbo");
        assert_eq!(json["text"], "hello");
        assert_eq!(json["stream"], false);
        assert_eq!(json["language_boost"], "Chinese,Yue");
        assert_eq!(json["output_format"], "hex");
        assert_eq!(json["voice_setting"]["voice_id"], "voice-123");
        assert_eq!(json["pronunciation_dict"]["tone"][0], "read/(riːd)");
        assert_eq!(json["audio_setting"]["format"], "pcm");
        assert_eq!(json["audio_setting"]["sample_rate"], 16000);
        assert_eq!(json["subtitle_enable"], true);
    }

    #[test]
    fn normalizes_language_tags_for_language_boost() {
        assert_eq!(normalize_language_boost("en-US"), "English");
        assert_eq!(normalize_language_boost("zh-CN"), "Chinese");
        assert_eq!(normalize_language_boost("yue-HK"), "Chinese,Yue");
        assert_eq!(normalize_language_boost("unknown"), "auto");
    }

    #[test]
    fn rejects_output_and_settings_incompatible_with_the_native_pipeline() {
        assert!(MiniMaxTts::new(
            MiniMaxTtsConfig::new("test-key").with_output_format(MiniMaxAudioOutputFormat::Mp3)
        )
        .is_err());

        let mut config = MiniMaxTtsConfig::new("test-key");
        config.sample_rate = 48_000;
        assert!(MiniMaxTts::new(config).is_err());
    }

    #[test]
    fn parses_hex_audio_response_and_validates_base_response() {
        let speech = parse_tts_response(
            r#"{"data":{"audio":"00000100","status":2},"base_resp":{"status_code":0}}"#,
            16_000,
            1,
        )
        .unwrap();

        assert_eq!(speech.audio, vec![0, 0, 1, 0]);
        assert_eq!(speech.format.sample_rate, 16_000);
        assert_eq!(speech.format.channels, 1);
    }

    #[test]
    fn rejects_failed_or_malformed_responses() {
        assert!(parse_tts_response(
            r#"{"data":{"audio":"0000","status":2},"base_resp":{"status_code":1001,"status_msg":"bad request"}}"#,
            16_000,
            1,
        )
        .is_err());
        assert!(parse_tts_response(
            r#"{"data":{"audio":"0000","status":1},"base_resp":{"status_code":0}}"#,
            16_000,
            1,
        )
        .is_err());
        assert!(parse_tts_response(
            r#"{"data":{"audio":"xyz","status":2},"base_resp":{"status_code":0}}"#,
            16_000,
            1,
        )
        .is_err());
        assert!(parse_tts_response(
            r#"{"data":{"status":2},"base_resp":{"status_code":0}}"#,
            16_000,
            1,
        )
        .is_err());
        assert!(parse_tts_response(r#"{"data":{"audio":"0000","status":2}}"#, 16_000, 1,).is_err());
    }
}
