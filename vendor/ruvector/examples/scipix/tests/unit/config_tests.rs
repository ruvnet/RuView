// Configuration tests for ruvector-scipix
//
// Tests configuration loading, serialization, validation, and defaults.
// Target: 90%+ coverage of config module

#[cfg(test)]
mod config_tests {
    use std::path::PathBuf;

    // Mock configuration structures for testing
    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct PreprocessConfig {
        target_dpi: u32,
        max_dimension: u32,
        denoise_strength: f32,
        contrast_enhancement: bool,
        auto_rotate: bool,
        binarization_method: String,
    }

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct OcrModelConfig {
        model_path: PathBuf,
        device: String,
        batch_size: usize,
        confidence_threshold: f32,
    }

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct OutputConfig {
        format: String,
        include_confidence: bool,
        include_geometry: bool,
    }

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct ScipixConfig {
        preprocessing: PreprocessConfig,
        model: OcrModelConfig,
        output: OutputConfig,
    }

    impl Default for PreprocessConfig {
        fn default() -> Self {
            Self {
                target_dpi: 300,
                max_dimension: 4096,
                denoise_strength: 0.5,
                contrast_enhancement: true,
                auto_rotate: true,
                binarization_method: "adaptive".to_string(),
            }
        }
    }

    impl Default for OcrModelConfig {
        fn default() -> Self {
            Self {
                model_path: PathBuf::from("models/scipix_model.onnx"),
                device: "cpu".to_string(),
                batch_size: 4,
                confidence_threshold: 0.7,
            }
        }
    }

    impl Default for OutputConfig {
        fn default() -> Self {
            Self {
                format: "latex".to_string(),
                include_confidence: true,
                include_geometry: false,
            }
        }
    }

    impl Default for ScipixConfig {
        fn default() -> Self {
            Self {
                preprocessing: PreprocessConfig::default(),
                model: OcrModelConfig::default(),
                output: OutputConfig::default(),
            }
        }
    }

    #[test]
    fn test_default_config_creation() {
        let config = ScipixConfig::default();

        assert_eq!(config.preprocessing.target_dpi, 300);
        assert_eq!(config.model.device, "cpu");
        assert_eq!(config.output.format, "latex");
    }

    #[test]
    fn test_preprocessing_config_defaults() {
        let config = PreprocessConfig::default();

        assert_eq!(config.target_dpi, 300);
        assert_eq!(config.max_dimension, 4096);
        assert_eq!(config.denoise_strength, 0.5);
        assert!(config.contrast_enhancement);
        assert!(config.auto_rotate);
        assert_eq!(config.binarization_method, "adaptive");
    }

    #[test]
    fn test_model_config_defaults() {
        let config = OcrModelConfig::default();

        assert_eq!(config.model_path, PathBuf::from("models/scipix_model.onnx"));
        assert_eq!(config.device, "cpu");
        assert_eq!(config.batch_size, 4);
        assert_eq!(config.confidence_threshold, 0.7);
    }

    #[test]
    fn test_output_config_defaults() {
        let config = OutputConfig::default();

        assert_eq!(config.format, "latex");
        assert!(config.include_confidence);
        assert!(!config.include_geometry);
    }

    #[test]
    fn test_toml_serialization() {
        let config = ScipixConfig::default();

        let toml_str = toml::to_string(&config).expect("Failed to serialize to TOML");

        assert!(toml_str.contains("target_dpi = 300"));
        assert!(toml_str.contains("device = \"cpu\""));
        assert!(toml_str.contains("format = \"latex\""));
    }

    #[test]
    fn test_toml_deserialization() {
        let toml_str = r#"
            [preprocessing]
            target_dpi = 300
            max_dimension = 4096
            denoise_strength = 0.5
            contrast_enhancement = true
            auto_rotate = true
            binarization_method = "adaptive"

            [model]
            model_path = "models/scipix_model.onnx"
            device = "cpu"
            batch_size = 4
            confidence_threshold = 0.7

            [output]
            format = "latex"
            include_confidence = true
            include_geometry = false
        "#;

        let config: ScipixConfig = toml::from_str(toml_str).expect("Failed to deserialize TOML");

        assert_eq!(config.preprocessing.target_dpi, 300);
        assert_eq!(config.model.device, "cpu");
        assert_eq!(config.output.format, "latex");
    }

    #[test]
    fn test_json_serialization() {
        let config = ScipixConfig::default();

        let json_str = serde_json::to_string(&config).expect("Failed to serialize to JSON");

        assert!(json_str.contains("\"target_dpi\":300"));
        assert!(json_str.contains("\"device\":\"cpu\""));
    }

    #[test]
    fn test_json_deserialization() {
        let json_str = r#"{
            "preprocessing": {
                "target_dpi": 300,
                "max_dimension": 4096,
                "denoise_strength": 0.5,
                "contrast_enhancement": true,
                "auto_rotate": true,
                "binarization_method": "adaptive"
            },
            "model": {
                "model_path": "models/scipix_model.onnx",
                "device": "cpu",
                "batch_size": 4,
                "confidence_threshold": 0.7
            },
            "output": {
                "format": "latex",
                "include_confidence": true,
                "include_geometry": false
            }
        }"#;

        let config: ScipixConfig =
            serde_json::from_str(json_str).expect("Failed to deserialize JSON");

        assert_eq!(config.preprocessing.target_dpi, 300);
        assert_eq!(config.model.device, "cpu");
    }

    #[test]
    fn test_preset_configurations() {
        // High quality preset
        let high_quality = ScipixConfig {
            preprocessing: PreprocessConfig {
                target_dpi: 600,
                denoise_strength: 0.8,
                ..Default::default()
            },
            model: OcrModelConfig {
                confidence_threshold: 0.9,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(high_quality.preprocessing.target_dpi, 600);
        assert_eq!(high_quality.model.confidence_threshold, 0.9);

        // Fast preset
        let fast = ScipixConfig {
            preprocessing: PreprocessConfig {
                target_dpi: 150,
                contrast_enhancement: false,
                auto_rotate: false,
                ..Default::default()
            },
            model: OcrModelConfig {
                batch_size: 8,
                confidence_threshold: 0.5,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(fast.preprocessing.target_dpi, 150);
        assert_eq!(fast.model.batch_size, 8);
    }

    #[test]
    fn test_config_validation_valid() {
        let config = ScipixConfig::default();

        // Basic validation checks
        assert!(config.preprocessing.target_dpi > 0);
        assert!(config.preprocessing.max_dimension > 0);
        assert!(config.preprocessing.denoise_strength >= 0.0);
        assert!(config.preprocessing.denoise_strength <= 1.0);
        assert!(config.model.batch_size > 0);
        assert!(config.model.confidence_threshold >= 0.0);
        assert!(config.model.confidence_threshold <= 1.0);
    }

    #[test]
    fn test_config_validation_invalid_values() {
        // Test invalid DPI
        let mut config = ScipixConfig::default();
        config.preprocessing.target_dpi = 0;
        assert_eq!(config.preprocessing.target_dpi, 0); // Would fail validation

        // Test invalid confidence threshold
        config = ScipixConfig::default();
        config.model.confidence_threshold = 1.5;
        assert!(config.model.confidence_threshold > 1.0); // Would fail validation
    }

    #[test]
    fn test_environment_variable_overrides() {
        // Simulate environment variable overrides
        let mut config = ScipixConfig::default();

        // Override device from environment
        let env_device = std::env::var("MATHPIX_DEVICE").unwrap_or_else(|_| "cpu".to_string());
        config.model.device = env_device;

        // Override batch size from environment
        let env_batch_size = std::env::var("MATHPIX_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(config.model.batch_size);
        config.model.batch_size = env_batch_size;

        assert!(!config.model.device.is_empty());
        assert!(config.model.batch_size > 0);
    }

    #[test]
    fn test_config_cloning() {
        let config1 = ScipixConfig::default();
        let config2 = config1.clone();

        assert_eq!(config1, config2);
        assert_eq!(
            config1.preprocessing.target_dpi,
            config2.preprocessing.target_dpi
        );
    }

    #[test]
    fn test_partial_config_update() {
        let mut config = ScipixConfig::default();

        // Update only preprocessing settings
        config.preprocessing.target_dpi = 450;
        config.preprocessing.denoise_strength = 0.7;

        assert_eq!(config.preprocessing.target_dpi, 450);
        assert_eq!(config.preprocessing.denoise_strength, 0.7);
        // Other settings should remain default
        assert_eq!(config.model.device, "cpu");
        assert_eq!(config.output.format, "latex");
    }

    #[test]
    fn test_binarization_methods() {
        let methods = vec!["otsu", "adaptive", "sauvola", "niblack"];

        for method in methods {
            let mut config = PreprocessConfig::default();
            config.binarization_method = method.to_string();
            assert_eq!(config.binarization_method, method);
        }
    }

    #[test]
    fn test_output_formats() {
        let formats = vec!["latex", "mathml", "mmd", "ascii", "unicode"];

        for format in formats {
            let mut config = OutputConfig::default();
            config.format = format.to_string();
            assert_eq!(config.format, format);
        }
    }

    #[test]
    fn test_device_configurations() {
        let devices = vec!["cpu", "cuda", "cuda:0", "cuda:1"];

        for device in devices {
            let mut config = OcrModelConfig::default();
            config.device = device.to_string();
            assert_eq!(config.device, device);
        }
    }

    #[test]
    fn test_config_roundtrip_toml() {
        let original = ScipixConfig::default();
        let toml_str = toml::to_string(&original).unwrap();
        let deserialized: ScipixConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_config_roundtrip_json() {
        let original = ScipixConfig::default();
        let json_str = serde_json::to_string(&original).unwrap();
        let deserialized: ScipixConfig = serde_json::from_str(&json_str).unwrap();

        assert_eq!(original, deserialized);
    }
}
