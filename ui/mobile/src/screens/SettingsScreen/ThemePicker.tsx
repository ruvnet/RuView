import { Pressable, View } from 'react-native';
import { ThemeMode } from '@/theme/ThemeContext';
import { ThemedText } from '@/components/ThemedText';
import { colors } from '@/theme/colors';
import { spacing } from '@/theme/spacing';

type ThemePickerProps = {
  value: ThemeMode;
  onChange: (value: ThemeMode) => void;
};

const OPTIONS: ThemeMode[] = ['light', 'dark', 'system'];

export const ThemePicker = ({ value, onChange }: ThemePickerProps) => {
  return (
    <View
      style={{
        flexDirection: 'row',
        gap: spacing.sm,
        marginTop: spacing.sm,
      }}
    >
      {OPTIONS.map((option) => {
        const isActive = option === value;
        return (
          <Pressable
            key={option}
            onPress={() => onChange(option)}
            style={{
              flex: 1,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: isActive ? colors.accent : colors.border,
              backgroundColor: isActive ? `${colors.accent}22` : '#0D1117',
              paddingVertical: 10,
              alignItems: 'center',
            }}
          >
            <ThemedText preset="labelMd" style={{ color: isActive ? colors.accent : colors.textSecondary }}>
              {option.toUpperCase()}
            </ThemedText>
          </Pressable>
        );
      })}
    </View>
  );
};
