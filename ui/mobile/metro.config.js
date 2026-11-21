const { getDefaultConfig } = require('expo/metro-config');

const config = getDefaultConfig(__dirname);

// Force CJS resolution for packages that use import.meta (not supported in Hermes script mode)
config.resolver = {
  ...config.resolver,
  unstable_enablePackageExports: false,
  // WebView screens import their HTML as a string. Metro treats .html as an
  // asset by default, which would hand them an asset id instead.
  assetExts: config.resolver.assetExts.filter((ext) => ext !== 'html'),
  sourceExts: [...config.resolver.sourceExts, 'html'],
};

config.transformer = {
  ...config.transformer,
  babelTransformerPath: require.resolve('./metro.html-transformer'),
};

module.exports = config;
