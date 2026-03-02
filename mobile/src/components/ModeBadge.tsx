import { StyleSheet } from 'react-native';
import { ThemedText } from './ThemedText';
import { colors } from '../theme/colors';

type Mode = 'CSI' | 'RSSI' | 'SIM' | 'LIVE';

const modeStyle: Record<
  Mode,
  {
    background: string;
    border: string;
    color: string;
  }
> = {
  CSI: {
    background: 'rgba(50, 184, 198, 0.25)',
    border: colors.accent,
    color: colors.accent,
  },
  RSSI: {
    background: 'rgba(255, 165, 2, 0.2)',
    border: colors.warn,
    color: colors.warn,
  },
  SIM: {
    background: 'rgba(255, 71, 87, 0.18)',
    border: colors.simulated,
    color: colors.simulated,
  },
  LIVE: {
    background: 'rgba(46, 213, 115, 0.18)',
    border: colors.connected,
    color: colors.connected,
  },
};

type ModeBadgeProps = {
  mode: Mode;
};

export const ModeBadge = ({ mode }: ModeBadgeProps) => {
  const style = modeStyle[mode];

  return (
    <ThemedText
      preset="labelMd"
      style={[
        styles.badge,
        {
          backgroundColor: style.background,
          borderColor: style.border,
          color: style.color,
        },
      ]}
    >
      {mode}
    </ThemedText>
  );
};

const styles = StyleSheet.create({
  badge: {
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 999,
    borderWidth: 1,
    overflow: 'hidden',
    letterSpacing: 1,
    textAlign: 'center',
  },
});
