import { StyleSheet, View } from 'react-native';
import { ThemedText } from './ThemedText';

type ConnectionState = 'connected' | 'simulated' | 'disconnected';

type ConnectionBannerProps = {
  status: ConnectionState;
};

const resolveState = (status: ConnectionState) => {
  if (status === 'connected') {
    return {
      label: 'LIVE STREAM',
      backgroundColor: '#0F6B2A',
      textColor: '#E2FFEA',
    };
  }

  if (status === 'disconnected') {
    return {
      label: 'DISCONNECTED',
      backgroundColor: '#8A1E2A',
      textColor: '#FFE3E7',
    };
  }

  return {
    label: 'SIMULATED DATA',
    backgroundColor: '#9A5F0C',
    textColor: '#FFF3E1',
  };
};

export const ConnectionBanner = ({ status }: ConnectionBannerProps) => {
  const state = resolveState(status);

  return (
    <View
      style={[
        styles.banner,
        {
          backgroundColor: state.backgroundColor,
          borderBottomColor: state.textColor,
        },
      ]}
    >
      <ThemedText preset="labelMd" style={[styles.text, { color: state.textColor }]}>
        {state.label}
      </ThemedText>
    </View>
  );
};

const styles = StyleSheet.create({
  banner: {
    position: 'absolute',
    left: 0,
    right: 0,
    top: 0,
    zIndex: 100,
    paddingVertical: 6,
    borderBottomWidth: 2,
    alignItems: 'center',
    justifyContent: 'center',
  },
  text: {
    letterSpacing: 2,
    fontWeight: '700',
  },
});
