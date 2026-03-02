import { ComponentPropsWithoutRef } from 'react';
import { StyleProp, Text, TextStyle } from 'react-native';
import { useTheme } from '../hooks/useTheme';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';

type TextPreset = keyof typeof typography;
type ColorKey = keyof typeof colors;

type ThemedTextProps = Omit<ComponentPropsWithoutRef<typeof Text>, 'style'> & {
  preset?: TextPreset;
  color?: ColorKey;
  style?: StyleProp<TextStyle>;
};

export const ThemedText = ({
  preset = 'bodyMd',
  color = 'textPrimary',
  style,
  ...props
}: ThemedTextProps) => {
  const { colors, typography } = useTheme();

  const presetStyle = (typography as Record<TextPreset, TextStyle>)[preset];
  const colorStyle = { color: colors[color] };

  return <Text {...props} style={[presetStyle, colorStyle, style]} />;
};
