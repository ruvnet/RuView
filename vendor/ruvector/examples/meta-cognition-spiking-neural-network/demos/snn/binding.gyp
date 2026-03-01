{
  "targets": [
    {
      "target_name": "snn_simd",
      "sources": ["native/snn_simd.cpp"],
      "include_dirs": [
        "<!@(node -p \"require('node-addon-api').include\")"
      ],
      "dependencies": [
        "<!(node -p \"require('node-addon-api').gyp\")"
      ],
      "cflags!": ["-fno-exceptions"],
      "cflags_cc!": ["-fno-exceptions"],
      "cflags": ["-msse4.1", "-mavx", "-O3", "-ffast-math"],
      "cflags_cc": ["-msse4.1", "-mavx", "-O3", "-ffast-math"],
      "defines": ["NAPI_DISABLE_CPP_EXCEPTIONS"],
      "xcode_settings": {
        "GCC_ENABLE_CPP_EXCEPTIONS": "YES",
        "CLANG_CXX_LIBRARY": "libc++",
        "MACOSX_DEPLOYMENT_TARGET": "10.7",
        "OTHER_CFLAGS": ["-msse4.1", "-mavx", "-O3", "-ffast-math"]
      },
      "msvs_settings": {
        "VCCLCompilerTool": {
          "ExceptionHandling": 1,
          "AdditionalOptions": ["/arch:AVX", "/O2"]
        }
      }
    }
  ]
}
