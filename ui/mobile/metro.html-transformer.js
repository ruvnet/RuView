// Loads imported .html files as string modules so the WebView screens can pass
// them as `source={{ html }}`. Everything else goes to Expo's default transformer.
const upstream = require(
  require.resolve('@expo/metro-config/build/babel-transformer', {
    paths: [require.resolve('expo/metro-config')],
  }),
);

module.exports = {
  ...upstream,
  transform(params) {
    if (params.filename.endsWith('.html')) {
      return upstream.transform({
        ...params,
        src: `export default ${JSON.stringify(params.src)};`,
      });
    }
    return upstream.transform(params);
  },
};
